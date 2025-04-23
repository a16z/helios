use std::{borrow::BorrowMut, collections::HashMap, mem, sync::Arc};

use alloy::{
    consensus::BlockHeader,
    network::{primitives::HeaderResponse, BlockResponse, TransactionBuilder},
    rpc::types::{
        AccessListResult, Block, EIP1186StorageProof, Header, Transaction, TransactionRequest,
    },
};
use eyre::{Report, Result};
use revm::{
    primitives::{
        address, AccessListItem, AccountInfo, Address, BlobExcessGasAndPrice, BlockEnv, Bytecode,
        Bytes, CfgEnv, Env, ExecutionResult, TxEnv, B256, U256,
    },
    Database, Evm as Revm,
};
use tracing::trace;

use helios_common::{
    execution_spec::ExecutionSpec,
    fork_schedule::ForkSchedule,
    network_spec::NetworkSpec,
    types::{AccessListResultWithAccounts, Account, BlockTag, EvmError},
};
use helios_core::execution::errors::ExecutionError;

use crate::spec::Ethereum;

pub struct Evm {
    execution: Arc<dyn ExecutionSpec<Ethereum>>,
    chain_id: u64,
    tag: BlockTag,
    fork_schedule: ForkSchedule,
}

impl Evm {
    pub fn new(
        execution: Arc<dyn ExecutionSpec<Ethereum>>,
        chain_id: u64,
        fork_schedule: ForkSchedule,
        tag: BlockTag,
    ) -> Self {
        Evm {
            execution,
            chain_id,
            tag,
            fork_schedule,
        }
    }

    pub async fn call(&mut self, tx: &TransactionRequest) -> Result<Bytes, EvmError> {
        let (result, ..) = self.call_inner(tx, false).await?;

        match result {
            ExecutionResult::Success { output, .. } => Ok(output.into_data()),
            ExecutionResult::Revert { output, .. } => {
                Err(EvmError::Revert(Some(output.to_vec().into())))
            }
            ExecutionResult::Halt { .. } => Err(EvmError::Revert(None)),
        }
    }

    pub async fn estimate_gas(&mut self, tx: &TransactionRequest) -> Result<u64, EvmError> {
        let (result, ..) = self.call_inner(tx, true).await?;

        match result {
            ExecutionResult::Success { gas_used, .. } => Ok(gas_used),
            ExecutionResult::Revert { gas_used, .. } => Ok(gas_used),
            ExecutionResult::Halt { gas_used, .. } => Ok(gas_used),
        }
    }

    pub async fn create_access_list(
        &mut self,
        tx: &TransactionRequest,
        validate_tx: bool,
    ) -> Result<AccessListResultWithAccounts, EvmError> {
        let (result, accounts) = self.call_inner(tx, validate_tx).await?;

        Ok(AccessListResultWithAccounts {
            access_list_result: AccessListResult {
                access_list: accounts
                    .iter()
                    .map(|(address, account)| {
                        let storage_keys = account
                            .storage_proof
                            .iter()
                            .map(|EIP1186StorageProof { key, .. }| key.as_b256())
                            .collect();
                        AccessListItem {
                            address: *address,
                            storage_keys,
                        }
                    })
                    .collect::<Vec<_>>()
                    .into(),
                gas_used: U256::from(result.gas_used()),
                error: matches!(result, ExecutionResult::Revert { .. })
                    .then_some(result.output().unwrap().to_string()),
            },
            accounts,
        })
    }

    async fn call_inner(
        &mut self,
        tx: &TransactionRequest,
        validate_tx: bool,
    ) -> Result<(ExecutionResult, HashMap<Address, Account>), EvmError> {
        let mut db = ProofDB::new(self.tag, self.execution.clone());
        _ = db.state.prefetch_state(tx, validate_tx).await;

        let env = Box::new(self.get_env(tx, self.tag, validate_tx).await);
        let evm = Revm::builder().with_db(db).with_env(env).build();
        let mut ctx = evm.into_context_with_handler_cfg();

        let tx_res = loop {
            let db = ctx.context.evm.db.borrow_mut();
            if db.state.needs_update() {
                db.state.update_state().await.unwrap();
            }

            let mut evm = Revm::builder().with_context_with_handler_cfg(ctx).build();
            let res = evm.transact();
            ctx = evm.into_context_with_handler_cfg();

            let db = ctx.context.evm.db.borrow_mut();
            let needs_update = db.state.needs_update();

            if res.is_ok() || !needs_update {
                break res.map(|res| (res.result, mem::take(&mut db.state.accounts)));
            }
        };

        tx_res.map_err(|err| EvmError::Generic(format!("generic: {}", err)))
    }

    async fn get_env(&self, tx: &TransactionRequest, tag: BlockTag, validate_tx: bool) -> Env {
        let block = self
            .execution
            .get_block(tag.into(), false)
            .await
            .unwrap()
            .ok_or(ExecutionError::BlockNotFound(tag))
            .unwrap();

        let mut cfg = CfgEnv::default();
        cfg.chain_id = self.chain_id;
        cfg.disable_block_gas_limit = !validate_tx;
        cfg.disable_eip3607 = !validate_tx;
        cfg.disable_base_fee = !validate_tx;

        Env {
            tx: Self::tx_env(tx),
            block: Self::block_env(&block, &self.fork_schedule),
            cfg,
        }
    }

    fn tx_env(tx: &TransactionRequest) -> TxEnv {
        TxEnv {
            caller: tx.from.unwrap_or_default(),
            gas_limit: <TransactionRequest as TransactionBuilder<Ethereum>>::gas_limit(tx)
                .unwrap_or(u64::MAX),
            gas_price: <TransactionRequest as TransactionBuilder<Ethereum>>::gas_price(tx)
                .map(U256::from)
                .unwrap_or_default(),
            transact_to: tx.to.unwrap_or_default(),
            value: tx.value.unwrap_or_default(),
            data: <TransactionRequest as TransactionBuilder<Ethereum>>::input(tx)
                .unwrap_or_default()
                .clone(),
            nonce: <TransactionRequest as TransactionBuilder<Ethereum>>::nonce(tx),
            chain_id: <TransactionRequest as TransactionBuilder<Ethereum>>::chain_id(tx),
            access_list: <TransactionRequest as TransactionBuilder<Ethereum>>::access_list(tx)
                .map(|v| v.to_vec())
                .unwrap_or_default(),
            gas_priority_fee:
                <TransactionRequest as TransactionBuilder<Ethereum>>::max_priority_fee_per_gas(tx)
                    .map(U256::from),
            max_fee_per_blob_gas: tx.max_fee_per_blob_gas.map(U256::from),
            blob_hashes: tx
                .blob_versioned_hashes
                .as_ref()
                .map(|v| v.to_vec())
                .unwrap_or_default(),
            authorization_list: None,
        }
    }

    fn block_env(block: &Block<Transaction, Header>, fork_schedule: &ForkSchedule) -> BlockEnv {
        let is_prague = block.header.timestamp >= fork_schedule.prague_timestamp;
        let blob_excess_gas_and_price = block
            .header
            .excess_blob_gas()
            .map(|v| BlobExcessGasAndPrice::new(v, is_prague))
            .unwrap_or_else(|| BlobExcessGasAndPrice::new(0, is_prague));

        BlockEnv {
            number: U256::from(block.header.number()),
            coinbase: block.header.beneficiary(),
            timestamp: U256::from(block.header.timestamp()),
            gas_limit: U256::from(block.header.gas_limit()),
            basefee: U256::from(block.header.base_fee_per_gas().unwrap_or(0_u64)),
            difficulty: block.header.difficulty(),
            prevrandao: block.header.mix_hash(),
            blob_excess_gas_and_price: Some(blob_excess_gas_and_price),
        }
    }
}

pub struct ProofDB<N: NetworkSpec> {
    state: EvmState<N>,
}

impl<N: NetworkSpec> ProofDB<N> {
    pub fn new(tag: BlockTag, execution: Arc<dyn ExecutionSpec<N>>) -> Self {
        let state = EvmState::new(execution, tag);
        ProofDB { state }
    }
}

enum StateAccess {
    Basic(Address),
    BlockHash(u64),
    Storage(Address, U256),
}

struct EvmState<N: NetworkSpec> {
    accounts: HashMap<Address, Account>,
    block_hash: HashMap<u64, B256>,
    block: BlockTag,
    access: Option<StateAccess>,
    execution: Arc<dyn ExecutionSpec<N>>,
}

impl<N: NetworkSpec> EvmState<N> {
    pub fn new(execution: Arc<dyn ExecutionSpec<N>>, block: BlockTag) -> Self {
        Self {
            execution,
            block,
            accounts: HashMap::new(),
            block_hash: HashMap::new(),
            access: None,
        }
    }

    pub async fn update_state(&mut self) -> Result<()> {
        if let Some(access) = self.access.take() {
            match access {
                StateAccess::Basic(address) => {
                    let account = self
                        .execution
                        .get_account(address, None, self.block, true)
                        .await?;

                    self.accounts.insert(address, account);
                }
                StateAccess::Storage(address, slot) => {
                    let slot_bytes = B256::from(slot);
                    let account = self
                        .execution
                        .get_account(address, Some(&[slot_bytes]), self.block, true)
                        .await?;

                    if let Some(stored_account) = self.accounts.get_mut(&address) {
                        if stored_account.code.is_none() {
                            stored_account.code = account.code;
                        }

                        for storage_proof in account.storage_proof {
                            stored_account.storage_proof.push(storage_proof);
                        }
                    } else {
                        self.accounts.insert(address, account);
                    }
                }
                StateAccess::BlockHash(number) => {
                    let tag = BlockTag::Number(number);
                    let block = self
                        .execution
                        .get_block(tag.into(), false)
                        .await?
                        .ok_or(ExecutionError::BlockNotFound(tag))?;

                    self.block_hash.insert(number, block.header().hash());
                }
            }
        }

        Ok(())
    }

    pub fn needs_update(&self) -> bool {
        self.access.is_some()
    }

    pub fn get_basic(&mut self, address: Address) -> Result<AccountInfo> {
        if let Some(account) = self.accounts.get(&address) {
            Ok(AccountInfo::new(
                account.account.balance,
                account.account.nonce,
                account.account.code_hash,
                Bytecode::new_raw(account.code.as_ref().unwrap().clone()),
            ))
        } else {
            self.access = Some(StateAccess::Basic(address));
            eyre::bail!("state missing");
        }
    }

    pub fn get_storage(&mut self, address: Address, slot: U256) -> Result<U256> {
        if let Some(account) = self.accounts.get(&address) {
            if let Some(value) = account.get_storage_value(B256::from(slot)) {
                return Ok(value);
            }
        }
        self.access = Some(StateAccess::Storage(address, slot));
        eyre::bail!("state missing");
    }

    pub fn get_block_hash(&mut self, block: u64) -> Result<B256> {
        if let Some(hash) = self.block_hash.get(&block) {
            Ok(*hash)
        } else {
            self.access = Some(StateAccess::BlockHash(block));
            eyre::bail!("state missing");
        }
    }

    pub async fn prefetch_state(
        &mut self,
        tx: &N::TransactionRequest,
        validate_tx: bool,
    ) -> Result<()> {
        let block_id = Some(self.block.into());
        let account_map = self
            .execution
            .create_extended_access_list(tx, validate_tx, block_id)
            .await
            .map_err(EvmError::RpcError)?;

        for (address, account) in account_map {
            self.accounts.insert(address, account);
        }

        Ok(())
    }
}

impl<N: NetworkSpec> Database for ProofDB<N> {
    type Error = Report;

    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Report> {
        if is_precompile(&address) {
            return Ok(Some(AccountInfo::default()));
        }

        trace!(
            target: "helios::evm",
            "fetch basic evm state for address=0x{}",
            hex::encode(address.as_slice())
        );

        Ok(Some(self.state.get_basic(address)?))
    }

    fn block_hash(&mut self, number: u64) -> Result<B256, Report> {
        trace!(target: "helios::evm", "fetch block hash for block={:?}", number);
        self.state.get_block_hash(number)
    }

    fn storage(&mut self, address: Address, slot: U256) -> Result<U256, Report> {
        trace!(target: "helios::evm", "fetch evm state for address={:?}, slot={}", address, slot);
        self.state.get_storage(address, slot)
    }

    fn code_by_hash(&mut self, _code_hash: B256) -> Result<Bytecode, Report> {
        Err(eyre::eyre!("should never be called"))
    }
}

fn is_precompile(address: &Address) -> bool {
    address.le(&address!("0000000000000000000000000000000000000009")) && address.gt(&Address::ZERO)
}

// #[cfg(test)]
// mod tests {
//     use revm::primitives::KECCAK_EMPTY;
//     use tokio::sync::{mpsc::channel, watch};
//
//     use crate::execution::{rpc::mock_rpc::MockRpc, state::State};
//
//     use super::*;
//
//     fn get_client() -> ExecutionClient<Ethereum, MockRpc> {
//         let (_, block_recv) = channel(256);
//         let (_, finalized_recv) = watch::channel(None);
//         let state = State::new(block_recv, finalized_recv, 64);
//         ExecutionClient::new("testdata/", state).unwrap()
//     }
//
//     #[tokio::test]
//     async fn test_proof_db() {
//         // Construct proofdb params
//         let execution = get_client();
//         let tag = BlockTag::Latest;
//
//         // Construct the proof database with the given client
//         let mut proof_db = ProofDB::new(tag, Arc::new(execution));
//
//         let address = address!("388C818CA8B9251b393131C08a736A67ccB19297");
//         let info = AccountInfo::new(
//             U256::from(500),
//             10,
//             KECCAK_EMPTY,
//             Bytecode::new_raw(revm::primitives::Bytes::default()),
//         );
//         proof_db.state.basic.insert(address, info.clone());
//
//         // Get the account from the proof database
//         let account = proof_db.basic(address).unwrap().unwrap();
//
//         assert_eq!(account, info);
//     }
// }
