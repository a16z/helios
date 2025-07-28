use std::{collections::HashMap, marker::PhantomData, mem, sync::Arc};

use alloy::{
    consensus::{BlockHeader, TxType},
    eips::BlockId,
    network::TransactionBuilder,
    rpc::types::{Block, Header, Transaction, TransactionRequest},
};
use eyre::Result;
use revm::{
    context::{result::ExecutionResult, BlockEnv, CfgEnv, ContextTr, TxEnv},
    context_interface::block::BlobExcessGasAndPrice,
    primitives::{hardfork::SpecId, Address},
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
    ) -> Result<(ExecutionResult, HashMap<Address, Account>), EvmError> {
        let mut db = ProofDB::new(self.block_id, self.execution.clone());
        _ = db.state.prefetch_state(tx, validate_tx).await;

        let mut evm = self
            .get_context(tx, self.block_id, validate_tx)
            .await?
            .with_db(db)
            .build_mainnet();

        let tx_res = loop {
            let db = evm.db();
            if db.state.needs_update() {
                debug!("evm cache miss: {:?}", db.state.access.as_ref().unwrap());
                db.state.update_state().await.unwrap();
            }

            let res = evm.replay();

            let db = evm.db();
            let needs_update = db.state.needs_update();

            if res.is_ok() || !needs_update {
                break res.map(|res| (res.result, mem::take(&mut db.state.accounts)));
            }
        };

        tx_res.map_err(|err| EvmError::Generic(format!("generic: {err}")))
    }

    async fn get_context(
        &self,
        tx: &TransactionRequest,
        block_id: BlockId,
        validate_tx: bool,
    ) -> Result<Context, EvmError> {
        let block = self
            .execution
            .get_block(block_id, false)
            .await
            .map_err(|err| EvmError::Generic(err.to_string()))?
            .ok_or(ExecutionError::BlockNotFound(block_id))
            .map_err(|err| EvmError::Generic(err.to_string()))?;

        let mut tx_env = Self::tx_env(tx);

        if <TxType as Into<u8>>::into(
            <TransactionRequest as TransactionBuilder<Ethereum>>::output_tx_type(tx),
        ) == 0u8
        {
            tx_env.chain_id = None;
        } else {
            tx_env.chain_id = Some(self.chain_id);
        }

        let mut cfg = CfgEnv::default();
        cfg.spec = get_spec_id_for_block_timestamp(block.header.timestamp, &self.fork_schedule);
        cfg.chain_id = self.chain_id;
        cfg.disable_block_gas_limit = !validate_tx;
        cfg.disable_eip3607 = !validate_tx;
        cfg.disable_base_fee = !validate_tx;
        cfg.disable_nonce_check = !validate_tx;

        Ok(Context::mainnet()
            .with_tx(tx_env)
            .with_block(Self::block_env(&block, &self.fork_schedule))
            .with_cfg(cfg))
    }

    fn tx_env(tx: &TransactionRequest) -> TxEnv {
        TxEnv {
            tx_type: tx.transaction_type.unwrap_or_default(),
            caller: tx.from.unwrap_or_default(),
            gas_limit: <TransactionRequest as TransactionBuilder<Ethereum>>::gas_limit(tx)
                .unwrap_or(u64::MAX),
            gas_price: <TransactionRequest as TransactionBuilder<Ethereum>>::gas_price(tx)
                .unwrap_or_default(),
            kind: tx.to.unwrap_or_default(),
            value: tx.value.unwrap_or_default(),
            data: <TransactionRequest as TransactionBuilder<Ethereum>>::input(tx)
                .unwrap_or_default()
                .clone(),
            nonce: <TransactionRequest as TransactionBuilder<Ethereum>>::nonce(tx)
                .unwrap_or_default(),
            chain_id: <TransactionRequest as TransactionBuilder<Ethereum>>::chain_id(tx),
            access_list: <TransactionRequest as TransactionBuilder<Ethereum>>::access_list(tx)
                .cloned()
                .unwrap_or_default(),
            gas_priority_fee:
                <TransactionRequest as TransactionBuilder<Ethereum>>::max_priority_fee_per_gas(tx),
            max_fee_per_blob_gas: tx.max_fee_per_blob_gas.unwrap_or_default(),
            blob_hashes: tx
                .blob_versioned_hashes
                .as_ref()
                .map(|v| v.to_vec())
                .unwrap_or_default(),
            authorization_list: vec![],
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
            number: block.header.number(),
            beneficiary: block.header.beneficiary(),
            timestamp: block.header.timestamp(),
            gas_limit: block.header.gas_limit(),
            basefee: block.header.base_fee_per_gas().unwrap_or_default(),
            difficulty: block.header.difficulty(),
            prevrandao: block.header.mix_hash(),
            blob_excess_gas_and_price: Some(blob_excess_gas_and_price),
        }
    }
}

pub fn get_spec_id_for_block_timestamp(timestamp: u64, fork_schedule: &ForkSchedule) -> SpecId {
    if timestamp >= fork_schedule.prague_timestamp {
        SpecId::PRAGUE
    } else if timestamp >= fork_schedule.cancun_timestamp {
        SpecId::CANCUN
    } else if timestamp >= fork_schedule.shanghai_timestamp {
        SpecId::SHANGHAI
    } else if timestamp >= fork_schedule.paris_timestamp {
        SpecId::MERGE
    } else if timestamp >= fork_schedule.gray_glacier_timestamp {
        SpecId::GRAY_GLACIER
    } else if timestamp >= fork_schedule.arrow_glacier_timestamp {
        SpecId::ARROW_GLACIER
    } else if timestamp >= fork_schedule.london_timestamp {
        SpecId::LONDON
    } else if timestamp >= fork_schedule.berlin_timestamp {
        SpecId::BERLIN
    } else if timestamp >= fork_schedule.muir_glacier_timestamp {
        SpecId::MUIR_GLACIER
    } else if timestamp >= fork_schedule.istanbul_timestamp {
        SpecId::ISTANBUL
    } else if timestamp >= fork_schedule.petersburg_timestamp {
        SpecId::PETERSBURG
    } else if timestamp >= fork_schedule.constantinople_timestamp {
        SpecId::CONSTANTINOPLE
    } else if timestamp >= fork_schedule.byzantium_timestamp {
        SpecId::BYZANTIUM
    } else if timestamp >= fork_schedule.spurious_dragon_timestamp {
        SpecId::SPURIOUS_DRAGON
    } else if timestamp >= fork_schedule.tangerine_timestamp {
        SpecId::TANGERINE
    } else if timestamp >= fork_schedule.dao_timestamp {
        SpecId::DAO_FORK
    } else if timestamp >= fork_schedule.homestead_timestamp {
        SpecId::HOMESTEAD
    } else if timestamp >= fork_schedule.frontier_timestamp {
        SpecId::FRONTIER
    } else {
        SpecId::default()
    }
}
