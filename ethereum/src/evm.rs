use std::{collections::HashMap, marker::PhantomData, mem, sync::Arc};

use alloy::{
    consensus::{BlockHeader, TxType},
    eips::{eip1898::RpcBlockHash, BlockId},
    network::TransactionBuilder,
    rpc::types::{state::StateOverride, Block, Header, Transaction, TransactionRequest},
};
use eyre::Result;
use revm::{
    context::{result::ExecutionResult, BlockEnv, CfgEnv, ContextTr, TxEnv},
    context_interface::{block::BlobExcessGasAndPrice, either::Either},
    primitives::{eip7825, hardfork::SpecId, Address, U256},
    Context, ExecuteEvm, MainBuilder, MainContext,
};
use tracing::debug;

use helios_common::{
    execution_provider::ExecutionProvider,
    fork_schedule::ForkSchedule,
    types::{Account, EvmError},
};
use helios_core::execution::errors::ExecutionError;
use helios_revm_utils::proof_db::ProofDB;

use crate::spec::Ethereum;

pub struct EthereumEvm<E: ExecutionProvider<Ethereum>> {
    execution: Arc<E>,
    chain_id: u64,
    block_id: BlockId,
    fork_schedule: ForkSchedule,
    phantom: PhantomData<Ethereum>,
}

impl<E: ExecutionProvider<Ethereum>> EthereumEvm<E> {
    pub fn new(
        execution: Arc<E>,
        chain_id: u64,
        fork_schedule: ForkSchedule,
        block_id: BlockId,
    ) -> Self {
        Self {
            execution,
            chain_id,
            block_id,
            fork_schedule,
            phantom: PhantomData,
        }
    }

    pub async fn transact_inner(
        &mut self,
        tx: &TransactionRequest,
        validate_tx: bool,
        state_overrides: Option<StateOverride>,
    ) -> Result<(ExecutionResult, HashMap<Address, Account>), EvmError> {
        let block = self
            .execution
            .get_block(self.block_id, false)
            .await
            .map_err(|err| EvmError::Generic(err.to_string()))?
            .ok_or(ExecutionError::BlockNotFound(self.block_id))
            .map_err(|err| EvmError::Generic(err.to_string()))?;

        // Pin block to a specific hash for the entire EVM run.
        let pinned_block: RpcBlockHash = block.header.hash.into();

        let mut db = ProofDB::new(pinned_block, self.execution.clone(), state_overrides);
        _ = db.state.prefetch_state(tx, validate_tx).await;

        // Track iterations for debugging
        let mut iteration: u32 = 0;

        let tx_res = loop {
            iteration += 1;

            // Update state first if needed
            if db.state.needs_update() {
                debug!(
                    "evm cache miss (iteration {}): {:?}",
                    iteration,
                    db.state.access.as_ref().unwrap()
                );
                db.state
                    .update_state()
                    .await
                    .map_err(|e| EvmError::Generic(e.to_string()))?;
            }

            // Create EVM after any async operations
            let context = self.get_context(tx, &block, validate_tx);

            // Execute in a scope to ensure EVM is dropped before any potential async operations
            let (result, needs_update) = {
                let mut evm = context.with_db(&mut db).build_mainnet();
                let res = evm.replay();
                let needs_update = evm.db_mut().state.needs_update();
                (res, needs_update)
            };

            if result.is_ok() || !needs_update {
                break result.map(|res| (res.result, mem::take(&mut db.state.accounts)));
            }
        };

        tx_res.map_err(|err| EvmError::Generic(format!("generic: {err}")))
    }

    fn get_context(
        &self,
        tx: &TransactionRequest,
        block: &Block<Transaction>,
        validate_tx: bool,
    ) -> Context {
        let spec = get_spec_id_for_block_timestamp(block.header.timestamp, &self.fork_schedule);
        let mut tx_env = Self::tx_env(tx, spec, block.header.gas_limit);

        if tx_env.tx_type == TxType::Legacy as u8 {
            tx_env.chain_id = None;
        } else {
            tx_env.chain_id = Some(self.chain_id);
        }

        let mut cfg = CfgEnv::new_with_spec(spec);
        cfg.chain_id = self.chain_id;
        cfg.disable_block_gas_limit = !validate_tx;
        cfg.disable_eip3607 = !validate_tx;
        cfg.disable_base_fee = !validate_tx;
        cfg.disable_nonce_check = !validate_tx;

        Context::mainnet()
            .with_tx(tx_env)
            .with_block(Self::block_env(block, &self.fork_schedule))
            .with_cfg(cfg)
    }

    fn tx_env(tx: &TransactionRequest, spec: SpecId, block_gas_limit: u64) -> TxEnv {
        let default_gas_limit = if spec >= SpecId::AMSTERDAM {
            // EIP-8037 caps execution gas, while state gas can use the remaining
            // transaction gas. REVM enforces the separate execution cap.
            block_gas_limit
        } else if spec >= SpecId::OSAKA {
            eip7825::TX_GAS_LIMIT_CAP
        } else {
            u64::MAX
        };
        TxEnv {
            tx_type: tx.transaction_type.unwrap_or(tx.minimal_tx_type() as u8),
            caller: tx.from.unwrap_or_default(),
            gas_limit: <TransactionRequest as TransactionBuilder>::gas_limit(tx)
                .unwrap_or(default_gas_limit),
            gas_price: tx.gas_price.or(tx.max_fee_per_gas).unwrap_or_default(),
            kind: tx.to.unwrap_or_default(),
            value: tx.value.unwrap_or_default(),
            data: <TransactionRequest as TransactionBuilder>::input(tx)
                .unwrap_or_default()
                .clone(),
            nonce: <TransactionRequest as TransactionBuilder>::nonce(tx).unwrap_or_default(),
            chain_id: <TransactionRequest as TransactionBuilder>::chain_id(tx),
            access_list: <TransactionRequest as TransactionBuilder>::access_list(tx)
                .cloned()
                .unwrap_or_default(),
            gas_priority_fee: <TransactionRequest as TransactionBuilder>::max_priority_fee_per_gas(
                tx,
            ),
            max_fee_per_blob_gas: tx.max_fee_per_blob_gas.unwrap_or_default(),
            blob_hashes: tx
                .blob_versioned_hashes
                .as_ref()
                .map(|v| v.to_vec())
                .unwrap_or_default(),
            authorization_list: tx
                .authorization_list
                .clone()
                .unwrap_or_default()
                .into_iter()
                .map(Either::Left)
                .collect(),
        }
    }

    fn block_env(block: &Block<Transaction, Header>, fork_schedule: &ForkSchedule) -> BlockEnv {
        // Get blob base fee update fraction based on fork
        let blob_base_fee_update_fraction =
            fork_schedule.get_blob_base_fee_update_fraction(block.header.timestamp());

        let blob_excess_gas_and_price = block
            .header
            .excess_blob_gas()
            .map(|v| BlobExcessGasAndPrice::new(v, blob_base_fee_update_fraction))
            .unwrap_or_else(|| BlobExcessGasAndPrice::new(0, blob_base_fee_update_fraction));

        BlockEnv {
            number: U256::from(block.header.number()),
            beneficiary: block.header.beneficiary(),
            timestamp: U256::from(block.header.timestamp()),
            gas_limit: block.header.gas_limit(),
            basefee: block.header.base_fee_per_gas().unwrap_or_default(),
            difficulty: block.header.difficulty(),
            prevrandao: block.header.mix_hash(),
            blob_excess_gas_and_price: Some(blob_excess_gas_and_price),
            slot_num: block.header.slot_number().unwrap_or_default(),
        }
    }
}

pub fn get_spec_id_for_block_timestamp(timestamp: u64, fork_schedule: &ForkSchedule) -> SpecId {
    if timestamp >= fork_schedule.amsterdam_timestamp {
        SpecId::AMSTERDAM
    } else if timestamp >= fork_schedule.osaka_timestamp {
        SpecId::OSAKA
    } else if timestamp >= fork_schedule.prague_timestamp {
        SpecId::PRAGUE
    } else if timestamp >= fork_schedule.cancun_timestamp {
        SpecId::CANCUN
    } else if timestamp >= fork_schedule.shanghai_timestamp {
        SpecId::SHANGHAI
    } else if timestamp >= fork_schedule.paris_timestamp {
        SpecId::MERGE
    } else if timestamp >= fork_schedule.london_timestamp {
        SpecId::LONDON
    } else if timestamp >= fork_schedule.berlin_timestamp {
        SpecId::BERLIN
    } else if timestamp >= fork_schedule.istanbul_timestamp {
        SpecId::ISTANBUL
    } else if timestamp >= fork_schedule.petersburg_timestamp {
        SpecId::PETERSBURG
    } else if timestamp >= fork_schedule.byzantium_timestamp {
        SpecId::BYZANTIUM
    } else if timestamp >= fork_schedule.spurious_dragon_timestamp {
        SpecId::SPURIOUS_DRAGON
    } else if timestamp >= fork_schedule.tangerine_timestamp {
        SpecId::TANGERINE
    } else if timestamp >= fork_schedule.homestead_timestamp {
        SpecId::HOMESTEAD
    } else if timestamp >= fork_schedule.frontier_timestamp {
        SpecId::FRONTIER
    } else {
        SpecId::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::address;
    use helios_core::execution::providers::{
        block::block_cache::BlockCache, rpc::RpcExecutionProvider,
    };
    use revm::{
        database::InMemoryDB,
        state::{AccountInfo, Bytecode},
    };
    fn test_evm() -> EthereumEvm<RpcExecutionProvider<Ethereum, BlockCache<Ethereum>, ()>> {
        let config = crate::config::networks::mainnet();
        let provider = RpcExecutionProvider::<Ethereum, _, ()>::new(
            "http://localhost:1".parse().unwrap(),
            BlockCache::new(),
            config.execution_forks,
        );
        EthereumEvm::new(
            Arc::new(provider),
            config.chain.chain_id,
            config.execution_forks,
            BlockId::latest(),
        )
    }

    #[tokio::test]
    async fn eip1559_call_uses_effective_gas_price() {
        let evm = test_evm();
        let caller = address!("1111111111111111111111111111111111111111");
        let recipient = address!("2222222222222222222222222222222222222222");
        let mut block: Block<Transaction> = Block::default();
        block.header.gas_limit = 100_000_000;
        block.header.timestamp = evm.fork_schedule.prague_timestamp;
        block.header.base_fee_per_gas = Some(5);
        let tx = TransactionRequest::default()
            .from(caller)
            .to(recipient)
            .max_fee_per_gas(20)
            .max_priority_fee_per_gas(2)
            .gas_limit(1_000_000);
        let mut db = InMemoryDB::default();
        db.insert_account_info(
            caller,
            AccountInfo::from_balance(U256::from(10_000_000_000u64)),
        );
        db.insert_account_info(
            recipient,
            AccountInfo::default().with_code(Bytecode::new_raw(alloy::primitives::bytes!(
                "3a60005260206000f3"
            ))),
        );
        let result = evm
            .get_context(&tx, &block, false)
            .with_db(db)
            .build_mainnet()
            .replay()
            .unwrap()
            .result;
        assert_eq!(
            result.output().unwrap().as_ref(),
            U256::from(7).to_be_bytes::<32>()
        );
    }

    #[tokio::test]
    async fn eip7702_call_executes_authorized_code() {
        use alloy::{eips::eip7702::Authorization, primitives::Signature};
        let evm = test_evm();
        let caller = address!("1111111111111111111111111111111111111111");
        let delegate = address!("2222222222222222222222222222222222222222");
        // Any recoverable signature defines an authority; no private key is needed for this test.
        let auth = Authorization {
            chain_id: U256::ZERO,
            address: delegate,
            nonce: 0,
        }
        .into_signed(Signature::new(U256::from(1), U256::from(2), false));
        let authority = auth.recover_authority().unwrap();
        let mut block: Block<Transaction> = Block::default();
        block.header.gas_limit = 100_000_000;
        block.header.timestamp = evm.fork_schedule.prague_timestamp;
        let tx = TransactionRequest {
            authorization_list: Some(vec![auth]),
            transaction_type: Some(4),
            gas: Some(1_000_000),
            ..TransactionRequest::default().from(caller).to(authority)
        };
        let mut db = InMemoryDB::default();
        db.insert_account_info(
            delegate,
            AccountInfo::default().with_code(Bytecode::new_raw(alloy::primitives::bytes!(
                "602a60005260206000f3"
            ))),
        );
        let result = evm
            .get_context(&tx, &block, false)
            .with_db(db)
            .build_mainnet()
            .replay()
            .unwrap()
            .result;
        assert_eq!(
            result.output().unwrap().as_ref(),
            U256::from(42).to_be_bytes::<32>()
        );
    }
    #[test]
    fn spec_id_selects_amsterdam_after_activation() {
        let fork_schedule = ForkSchedule {
            prague_timestamp: 10,
            osaka_timestamp: 20,
            amsterdam_timestamp: 30,
            ..Default::default()
        };

        assert_eq!(
            get_spec_id_for_block_timestamp(29, &fork_schedule),
            SpecId::OSAKA
        );
        assert_eq!(
            get_spec_id_for_block_timestamp(30, &fork_schedule),
            SpecId::AMSTERDAM
        );
    }
    #[tokio::test]
    async fn amsterdam_calls_use_final_gas_rules_and_slot_number() {
        let config = crate::config::networks::plataberget();
        let provider = RpcExecutionProvider::<Ethereum, _, ()>::new(
            "http://localhost:1".parse().unwrap(),
            BlockCache::new(),
            config.execution_forks,
        );
        let evm = EthereumEvm::new(
            Arc::new(provider),
            config.chain.chain_id,
            config.execution_forks,
            BlockId::latest(),
        );
        let caller = address!("1111111111111111111111111111111111111111");
        let recipient = address!("2222222222222222222222222222222222222222");
        for (timestamp, value, gas) in [
            (config.execution_forks.amsterdam_timestamp - 1, 0, 21_000),
            (config.execution_forks.amsterdam_timestamp, 0, 15_000),
            (config.execution_forks.amsterdam_timestamp, 1, 21_000),
        ] {
            let mut block: Block<Transaction> = Block::default();
            block.header.gas_limit = 100_000_000;
            block.header.timestamp = timestamp;
            let tx = TransactionRequest::default()
                .from(caller)
                .to(recipient)
                .value(U256::from(value));
            let mut db = InMemoryDB::default();
            db.insert_account_info(caller, AccountInfo::from_balance(U256::from(1_000_000)));
            db.insert_account_info(recipient, AccountInfo::from_balance(U256::from(1)));
            let mut vm = evm
                .get_context(&tx, &block, false)
                .with_db(db)
                .build_mainnet();
            let result = vm.replay().unwrap().result;
            assert!(result.is_success(), "{result:?}");
            assert_eq!(result.tx_gas_used(), gas);
        }

        let mut block: Block<Transaction> = Block::default();
        block.header.gas_limit = 100_000_000;
        block.header.timestamp = config.execution_forks.amsterdam_timestamp;
        block.header.slot_number = Some(49_152);
        let tx = TransactionRequest::default().from(caller).to(recipient);
        let mut db = InMemoryDB::default();
        // SLOTNUM; MSTORE(0); RETURN(0, 32)
        db.insert_account_info(
            recipient,
            AccountInfo::default().with_code(Bytecode::new_raw(alloy::primitives::bytes!(
                "4b60005260206000f3"
            ))),
        );
        let mut vm = evm
            .get_context(&tx, &block, false)
            .with_db(db)
            .build_mainnet();
        let result = vm.replay().unwrap().result;
        assert_eq!(
            result.output().unwrap().as_ref(),
            U256::from(49_152).to_be_bytes::<32>()
        );
    }

    #[tokio::test]
    async fn amsterdam_default_call_gas_allows_state_reservoir() {
        let mut evm = test_evm();
        evm.fork_schedule = crate::config::networks::plataberget().execution_forks;
        let recipient = address!("2222222222222222222222222222222222222222");
        let mut block: Block<Transaction> = Block::default();
        block.header.timestamp = evm.fork_schedule.amsterdam_timestamp;
        block.header.gas_limit = 100_000_000;
        let tx = TransactionRequest::default().to(recipient);
        // Fill distinct slots: execution gas stays below the cap but state gas
        // requires a total transaction gas allowance greater than 2^24.
        let mut code = Vec::new();
        for slot in 0u16..800 {
            code.extend_from_slice(&[0x60, 1, 0x61]);
            code.extend_from_slice(&slot.to_be_bytes());
            code.push(0x55);
        }
        let mut db = InMemoryDB::default();
        db.insert_account_info(
            recipient,
            AccountInfo::default().with_code(Bytecode::new_raw(code.into())),
        );
        let result = evm
            .get_context(&tx, &block, false)
            .with_db(db)
            .build_mainnet()
            .replay()
            .unwrap()
            .result;
        assert!(result.is_success(), "{result:?}");
        assert!(
            result.tx_gas_used() > eip7825::TX_GAS_LIMIT_CAP,
            "{result:?}"
        );
    }
}
