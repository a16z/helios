use std::{collections::HashMap, marker::PhantomData, mem, sync::Arc};

use alloy::{
    consensus::BlockHeader,
    eips::BlockId,
    network::TransactionBuilder,
    rpc::types::{Block, Header},
};
use eyre::Result;
use op_alloy_consensus::OpTxType;
use op_alloy_rpc_types::{OpTransactionRequest, Transaction};
use op_revm::{DefaultOp, OpBuilder, OpContext, OpHaltReason, OpSpecId, OpTransaction};
use revm::{
    context::{result::ExecutionResult, BlockEnv, CfgEnv, ContextTr, TxEnv},
    context_interface::block::BlobExcessGasAndPrice,
    database::EmptyDB,
    primitives::{Address, Bytes},
    Context, ExecuteEvm,
};
use tracing::debug;

use helios_common::{
    execution_provider::ExecutionProivder,
    fork_schedule::ForkSchedule,
    types::{Account, EvmError},
};
use helios_core::execution::errors::ExecutionError;
use helios_revm_utils::proof_db::ProofDB;

use crate::spec::OpStack;

pub struct OpStackEvm<E: ExecutionProivder<OpStack>> {
    execution: Arc<E>,
    chain_id: u64,
    block_id: BlockId,
    fork_schedule: ForkSchedule,
    phantom: PhantomData<OpStack>,
}

impl<E: ExecutionProivder<OpStack>> OpStackEvm<E> {
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
        tx: &OpTransactionRequest,
        validate_tx: bool,
    ) -> Result<(ExecutionResult<OpHaltReason>, HashMap<Address, Account>), EvmError> {
        let mut db = ProofDB::new(self.block_id, self.execution.clone());
        _ = db.state.prefetch_state(tx, validate_tx).await;

        let mut evm = self
            .get_context(tx, self.block_id, validate_tx)
            .await
            .with_db(db)
            .build_op();

        let tx_res = loop {
            let db = evm.0.db();
            if db.state.needs_update() {
                debug!("evm cache miss: {:?}", db.state.access.as_ref().unwrap());
                db.state.update_state().await.unwrap();
            }

            let res = evm.replay();

            let db = evm.0.db();
            let needs_update = db.state.needs_update();

            if res.is_ok() || !needs_update {
                break res.map(|res| (res.result, mem::take(&mut db.state.accounts)));
            }
        };

        tx_res.map_err(|err| EvmError::Generic(format!("generic: {err}")))
    }

    async fn get_context(
        &self,
        tx: &OpTransactionRequest,
        block_id: BlockId,
        validate_tx: bool,
    ) -> OpContext<EmptyDB> {
        let block = self
            .execution
            .get_block(block_id, false)
            .await
            .unwrap()
            .ok_or(ExecutionError::BlockNotFound(block_id))
            .unwrap();

        let mut tx_env = Self::tx_env(tx);

        if <OpTxType as Into<u8>>::into(
            <OpTransactionRequest as TransactionBuilder<OpStack>>::output_tx_type(tx),
        ) == 0u8
        {
            tx_env.chain_id = None;
        } else {
            tx_env.chain_id = Some(self.chain_id);
        }

        let mut cfg = CfgEnv::default();
        cfg.spec = get_spec_id_for_block_timestamp(block.header.timestamp(), &self.fork_schedule);
        cfg.chain_id = self.chain_id;
        cfg.disable_block_gas_limit = !validate_tx;
        cfg.disable_eip3607 = !validate_tx;
        cfg.disable_base_fee = !validate_tx;
        cfg.disable_nonce_check = !validate_tx;

        let mut op_tx_env = OpTransaction::new(tx_env);
        op_tx_env.enveloped_tx = Some(Bytes::new());

        Context::op()
            .with_tx(op_tx_env)
            .with_block(Self::block_env(&block, &self.fork_schedule))
            .with_cfg(cfg)
    }

    fn tx_env(tx: &OpTransactionRequest) -> TxEnv {
        TxEnv {
            tx_type: <OpTransactionRequest as TransactionBuilder<OpStack>>::output_tx_type(tx)
                .into(),
            caller: <OpTransactionRequest as TransactionBuilder<OpStack>>::from(tx)
                .unwrap_or_default(),
            gas_limit: <OpTransactionRequest as TransactionBuilder<OpStack>>::gas_limit(tx)
                .unwrap_or(u64::MAX),
            gas_price: <OpTransactionRequest as TransactionBuilder<OpStack>>::gas_price(tx)
                .unwrap_or_default(),
            kind: <OpTransactionRequest as TransactionBuilder<OpStack>>::kind(tx)
                .unwrap_or_default(),
            value: <OpTransactionRequest as TransactionBuilder<OpStack>>::value(tx)
                .unwrap_or_default(),
            data: <OpTransactionRequest as TransactionBuilder<OpStack>>::input(tx)
                .unwrap_or_default()
                .clone(),
            nonce: <OpTransactionRequest as TransactionBuilder<OpStack>>::nonce(tx)
                .unwrap_or_default(),
            chain_id: <OpTransactionRequest as TransactionBuilder<OpStack>>::chain_id(tx),
            access_list: <OpTransactionRequest as TransactionBuilder<OpStack>>::access_list(tx)
                .cloned()
                .unwrap_or_default(),
            gas_priority_fee:
                <OpTransactionRequest as TransactionBuilder<OpStack>>::max_priority_fee_per_gas(tx),
            max_fee_per_blob_gas: 0,
            blob_hashes: tx
                .as_ref()
                .blob_versioned_hashes
                .as_ref()
                .map(|v| v.to_vec())
                .unwrap_or_default(),
            authorization_list: vec![],
        }
    }

    fn block_env(block: &Block<Transaction, Header>, _fork_schedule: &ForkSchedule) -> BlockEnv {
        let blob_excess_gas_and_price = Some(BlobExcessGasAndPrice {
            excess_blob_gas: 0,
            blob_gasprice: 0,
        });

        BlockEnv {
            number: block.header.number(),
            beneficiary: block.header.beneficiary(),
            timestamp: block.header.timestamp(),
            gas_limit: block.header.gas_limit(),
            basefee: block.header.base_fee_per_gas().unwrap_or(0_u64),
            difficulty: block.header.difficulty(),
            prevrandao: block.header.mix_hash(),
            blob_excess_gas_and_price,
        }
    }
}

pub fn get_spec_id_for_block_timestamp(timestamp: u64, fork_schedule: &ForkSchedule) -> OpSpecId {
    if timestamp >= fork_schedule.isthmus_timestamp {
        OpSpecId::ISTHMUS
    } else if timestamp >= fork_schedule.holocene_timestamp {
        OpSpecId::HOLOCENE
    } else if timestamp >= fork_schedule.granite_timestamp {
        OpSpecId::GRANITE
    } else if timestamp >= fork_schedule.fjord_timestamp {
        OpSpecId::FJORD
    } else if timestamp >= fork_schedule.ecotone_timestamp {
        OpSpecId::ECOTONE
    } else if timestamp >= fork_schedule.canyon_timestamp {
        OpSpecId::CANYON
    } else if timestamp >= fork_schedule.regolith_timestamp {
        OpSpecId::REGOLITH
    } else if timestamp >= fork_schedule.bedrock_timestamp {
        OpSpecId::BEDROCK
    } else {
        OpSpecId::default()
    }
}
