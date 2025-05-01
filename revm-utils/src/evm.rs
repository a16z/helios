use std::{borrow::BorrowMut, sync::Arc};

use alloy::{
    consensus::BlockHeader,
    network::{BlockResponse, TransactionBuilder},
    rpc::types::AccessListResult,
};
use eyre::Result;
use revm::{
    primitives::{
        AccessListItem, Address, BlobExcessGasAndPrice, BlockEnv, Bytes, CfgEnv, Env,
        EnvWithHandlerCfg, ExecutionResult, HandlerCfg, OptimismFields, SpecId, TxEnv, U256,
    },
    Evm as Revm,
};

use helios_common::{
    execution_spec::ExecutionSpec,
    fork_schedule::ForkSchedule,
    network_spec::NetworkSpec,
    types::{AccessListResultWithAccounts, Account, BlockTag, EvmError},
};
use helios_core::execution::errors::ExecutionError;

use crate::{proof_db::ProofDB, types::RevmNetwork, utils::get_spec_id_for_block};

pub struct RevmExecutor<N: NetworkSpec> {
    execution: Arc<dyn ExecutionSpec<N>>,
    chain_id: u64,
    tag: BlockTag,
    network: RevmNetwork,
    fork_schedule: ForkSchedule,
}

impl<N: NetworkSpec> RevmExecutor<N> {
    pub fn new(
        execution: Arc<dyn ExecutionSpec<N>>,
        chain_id: u64,
        tag: BlockTag,
        network: RevmNetwork,
        fork_schedule: ForkSchedule,
    ) -> Self {
        Self {
            execution,
            chain_id,
            tag,
            network,
            fork_schedule,
        }
    }

    pub async fn call(&mut self, tx: &N::TransactionRequest) -> Result<Bytes, EvmError> {
        let (result, ..) = self.call_inner(tx, false).await?;

        match result {
            ExecutionResult::Success { output, .. } => Ok(output.into_data()),
            ExecutionResult::Revert { output, .. } => {
                Err(EvmError::Revert(Some(output.to_vec().into())))
            }
            ExecutionResult::Halt { .. } => Err(EvmError::Revert(None)),
        }
    }

    pub async fn estimate_gas(&mut self, tx: &N::TransactionRequest) -> Result<u64, EvmError> {
        let (result, ..) = self.call_inner(tx, true).await?;

        match result {
            ExecutionResult::Success { gas_used, .. } => Ok(gas_used),
            ExecutionResult::Revert { gas_used, .. } => Ok(gas_used),
            ExecutionResult::Halt { gas_used, .. } => Ok(gas_used),
        }
    }

    pub async fn create_access_list(
        &mut self,
        tx: &N::TransactionRequest,
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
                            .map(|storage_proof| storage_proof.key.as_b256())
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
        tx: &N::TransactionRequest,
        validate_tx: bool,
    ) -> Result<(ExecutionResult, std::collections::HashMap<Address, Account>), EvmError> {
        let mut db = ProofDB::new(self.tag, self.execution.clone());
        _ = db.state.prefetch_state(tx, validate_tx).await;

        let env_with_handler_cfg = self.get_env_with_handler_cfg(tx, validate_tx).await;
        let evm = Revm::builder()
            .with_db(db)
            .with_env_with_handler_cfg(env_with_handler_cfg)
            .build();

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
                break res.map(|res| (res.result, std::mem::take(&mut db.state.accounts)));
            }
        };

        tx_res.map_err(|err| EvmError::Generic(format!("generic: {}", err)))
    }

    async fn get_env_with_handler_cfg(
        &self,
        tx: &N::TransactionRequest,
        validate_tx: bool,
    ) -> EnvWithHandlerCfg {
        let block = self
            .execution
            .get_block(self.tag.into(), false)
            .await
            .unwrap()
            .ok_or(ExecutionError::BlockNotFound(self.tag))
            .unwrap();

        let is_optimism = matches!(self.network, RevmNetwork::OpStack);
        let spec_id = get_spec_id_for_block(
            &self.network,
            &self.fork_schedule,
            block.header().timestamp(),
        );
        let spec_id = spec_id.unwrap_or(SpecId::LATEST);
        let handler_cfg = HandlerCfg {
            spec_id,
            is_optimism,
        };

        let mut cfg = CfgEnv::default();
        cfg.chain_id = self.chain_id;
        cfg.disable_block_gas_limit = !validate_tx;
        cfg.disable_eip3607 = !validate_tx;
        cfg.disable_base_fee = !validate_tx;

        let env = Env {
            tx: Self::tx_env(tx),
            block: Self::block_env(&block, &self.fork_schedule, is_optimism),
            cfg,
        };

        EnvWithHandlerCfg::new(Box::new(env), handler_cfg)
    }

    fn tx_env(tx: &N::TransactionRequest) -> TxEnv {
        // TODO: Handle `max_fee_per_blob_gas`,`blob_hashes` and `optimism` fields
        TxEnv {
            caller: tx.from().unwrap_or_default(),
            gas_limit: tx.gas_limit().unwrap_or(u64::MAX),
            gas_price: tx.gas_price().map(U256::from).unwrap_or_default(),
            transact_to: tx.kind().unwrap_or_default(),
            value: tx.value().unwrap_or_default(),
            data: tx.input().unwrap_or_default().clone(),
            nonce: tx.nonce(),
            chain_id: tx.chain_id(),
            access_list: tx.access_list().map(|v| v.to_vec()).unwrap_or_default(),
            gas_priority_fee: tx.max_priority_fee_per_gas().map(U256::from),
            max_fee_per_blob_gas: None,
            blob_hashes: Vec::new(),
            authorization_list: None,
            optimism: OptimismFields::default(),
        }
    }

    fn block_env(
        block: &N::BlockResponse,
        fork_schedule: &ForkSchedule,
        is_optimism: bool,
    ) -> BlockEnv {
        let is_prague = block.header().timestamp() >= fork_schedule.prague_timestamp;

        let blob_excess_gas_and_price = if is_optimism {
            block
                .header()
                .excess_blob_gas()
                .map(|v| BlobExcessGasAndPrice::new(v, is_prague))
                .unwrap_or_else(|| BlobExcessGasAndPrice::new(0, is_prague))
        } else {
            BlobExcessGasAndPrice::new(0, is_prague)
        };

        BlockEnv {
            number: U256::from(block.header().number()),
            coinbase: block.header().beneficiary(),
            timestamp: U256::from(block.header().timestamp()),
            gas_limit: U256::from(block.header().gas_limit()),
            basefee: U256::from(block.header().base_fee_per_gas().unwrap_or(0_u64)),
            difficulty: block.header().difficulty(),
            prevrandao: block.header().mix_hash(),
            blob_excess_gas_and_price: Some(blob_excess_gas_and_price),
        }
    }
}
