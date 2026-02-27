use std::{collections::HashMap, marker::PhantomData, mem, sync::Arc};

use alloy::{
    consensus::BlockHeader,
    eips::BlockId,
    network::TransactionBuilder,
    rpc::types::{state::StateOverride, Block, Header},
};
use eyre::Result;
use op_alloy_consensus::OpTxType;
use op_alloy_rpc_types::{OpTransactionRequest, Transaction};
use op_revm::{DefaultOp, OpBuilder, OpContext, OpHaltReason, OpSpecId, OpTransaction};
use revm::{
    context::{result::ExecutionResult, BlockEnv, CfgEnv, ContextTr, TxEnv},
    context_interface::block::BlobExcessGasAndPrice,
    database::EmptyDB,
    primitives::{Address, Bytes, U256},
    Context, ExecuteEvm,
};
use tracing::debug;

use helios_common::{
    execution_provider::ExecutionProvider,
    fork_schedule::ForkSchedule,
    types::{Account, EvmError},
};
use helios_core::execution::errors::ExecutionError;
use helios_revm_utils::proof_db::ProofDB;

use crate::spec::OpStack;

pub struct OpStackEvm<E: ExecutionProvider<OpStack>> {
    execution: Arc<E>,
    chain_id: u64,
    block_id: BlockId,
    fork_schedule: ForkSchedule,
    phantom: PhantomData<OpStack>,
}

impl<E: ExecutionProvider<OpStack>> OpStackEvm<E> {
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
        state_overrides: Option<StateOverride>,
    ) -> Result<(ExecutionResult<OpHaltReason>, HashMap<Address, Account>), EvmError> {
        let block = self
            .execution
            .get_block(self.block_id, false)
            .await
            .map_err(|err| EvmError::Generic(err.to_string()))?
            .ok_or(ExecutionError::BlockNotFound(self.block_id))
            .map_err(|err| EvmError::Generic(err.to_string()))?;

        // Pin block id to a specific hash for the entire EVM run
        let pinned_block_id: BlockId = block.header.hash.into();

        let mut db = ProofDB::new(pinned_block_id, self.execution.clone(), state_overrides);
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
                let mut evm = context.with_db(&mut db).build_op();
                let res = evm.replay();
                let needs_update = evm.0.db_mut().state.needs_update();
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
        tx: &OpTransactionRequest,
        block: &Block<Transaction>,
        validate_tx: bool,
    ) -> OpContext<EmptyDB> {
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
        cfg.spec = get_spec_id_for_block_timestamp(block.header.timestamp, &self.fork_schedule);
        cfg.chain_id = self.chain_id;
        cfg.disable_block_gas_limit = !validate_tx;
        cfg.disable_eip3607 = !validate_tx;
        cfg.disable_base_fee = !validate_tx;
        cfg.disable_nonce_check = !validate_tx;

        let mut op_tx_env = OpTransaction::new(tx_env);
        op_tx_env.enveloped_tx = Some(Bytes::new());

        Context::op()
            .with_tx(op_tx_env)
            .with_block(Self::block_env(block, &self.fork_schedule))
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

    fn block_env(block: &Block<Transaction, Header>, fork_schedule: &ForkSchedule) -> BlockEnv {
        // Get blob base fee update fraction based on fork
        let blob_base_fee_update_fraction =
            fork_schedule.get_blob_base_fee_update_fraction(block.header.timestamp());

        let blob_excess_gas_and_price =
            Some(BlobExcessGasAndPrice::new(0, blob_base_fee_update_fraction));

        BlockEnv {
            number: U256::from(block.header.number()),
            beneficiary: block.header.beneficiary(),
            timestamp: U256::from(block.header.timestamp()),
            gas_limit: block.header.gas_limit(),
            basefee: block.header.base_fee_per_gas().unwrap_or(0_u64),
            difficulty: block.header.difficulty(),
            prevrandao: block.header.mix_hash(),
            blob_excess_gas_and_price,
        }
    }
}

pub fn get_spec_id_for_block_timestamp(timestamp: u64, fork_schedule: &ForkSchedule) -> OpSpecId {
    if timestamp >= fork_schedule.jovian_timestamp {
        OpSpecId::OSAKA
    } else if timestamp >= fork_schedule.isthmus_timestamp {
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
