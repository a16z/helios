use std::{future::Future, sync::Arc};

use alloy::{
    consensus::{
        proofs::{calculate_transaction_root, calculate_withdrawals_root},
        BlockHeader, Receipt, ReceiptWithBloom, TxReceipt, TxType, TypedTransaction,
    },
    network::{BuildResult, Network, NetworkWallet, TransactionBuilder, TransactionBuilderError},
    primitives::{Address, Bytes, ChainId, TxKind, U256},
    rpc::types::{AccessList, Log, TransactionRequest},
};
use revm::{
    primitives::{
        BlobExcessGasAndPrice, BlockEnv, Env, ExecutionResult, HandlerCfg, ResultAndState, SpecId,
        TxEnv,
    },
    EvmBuilder,
};

use helios_core::{
    execution::{
        errors::{EvmError, ExecutionError},
        evm::ProofDB,
        rpc::http_rpc::HttpRpc,
        ExecutionClient,
    },
    network_spec::NetworkSpec,
    types::BlockTag,
};

// use crate::rpc::http_rpc::HttpRpc;

#[derive(Clone, Copy, Debug)]
pub struct Ethereum;

impl Ethereum {
    fn evm(
        db: ProofDB<Self, HttpRpc<Self>>,
        env: Box<revm::primitives::Env>,
    ) -> revm::Evm<'static, (), ProofDB<Self, HttpRpc<Self>>> {
        EvmBuilder::default()
            .with_db(db)
            .with_handler_cfg(HandlerCfg::new(SpecId::LATEST))
            .with_env(env)
            .build()
    }

    fn tx_env(tx: &<Self as alloy::providers::Network>::TransactionRequest) -> TxEnv {
        let mut tx_env = TxEnv::default();
        tx_env.caller = tx.from.unwrap_or_default();
        tx_env.gas_limit = <TransactionRequest as TransactionBuilder<Self>>::gas_limit(tx)
            .map(|v| v as u64)
            .unwrap_or(u64::MAX);
        tx_env.gas_price = <TransactionRequest as TransactionBuilder<Self>>::gas_price(tx)
            .map(U256::from)
            .unwrap_or_default();
        tx_env.transact_to = tx.to.unwrap_or_default();
        tx_env.value = tx.value.unwrap_or_default();
        tx_env.data = <TransactionRequest as TransactionBuilder<Self>>::input(tx)
            .unwrap_or_default()
            .clone();
        tx_env.nonce = <TransactionRequest as TransactionBuilder<Self>>::nonce(tx);
        tx_env.chain_id = <TransactionRequest as TransactionBuilder<Self>>::chain_id(tx);
        tx_env.access_list = <TransactionRequest as TransactionBuilder<Self>>::access_list(tx)
            .map(|v| v.to_vec())
            .unwrap_or_default();
        tx_env.gas_priority_fee =
            <TransactionRequest as TransactionBuilder<Self>>::max_priority_fee_per_gas(tx)
                .map(U256::from);
        tx_env.max_fee_per_blob_gas =
            <TransactionRequest as TransactionBuilder<Self>>::max_fee_per_gas(tx).map(U256::from);
        tx_env.blob_hashes = tx
            .blob_versioned_hashes
            .as_ref()
            .map(|v| v.to_vec())
            .unwrap_or_default();

        tx_env
    }

    fn block_env(block: &<Self as alloy::providers::Network>::BlockResponse) -> BlockEnv {
        let mut block_env = BlockEnv::default();
        block_env.number = U256::from(block.header.number());
        block_env.coinbase = block.header.beneficiary();
        block_env.timestamp = U256::from(block.header.timestamp());
        block_env.gas_limit = U256::from(block.header.gas_limit());
        block_env.basefee = U256::from(block.header.base_fee_per_gas().unwrap_or(0_u64));
        block_env.difficulty = block.header.difficulty();
        block_env.prevrandao = block.header.mix_hash();
        block_env.blob_excess_gas_and_price = block
            .header
            .excess_blob_gas()
            .map(|v| BlobExcessGasAndPrice::new(v.into()));

        block_env
    }

    async fn call_inner(
        tx: &<Self as alloy::providers::Network>::TransactionRequest,
        execution: Arc<ExecutionClient<Self, HttpRpc<Self>>>,
        chain_id: u64,
        tag: helios_core::types::BlockTag,
    ) -> Result<ResultAndState, EvmError> {
        let mut db = ProofDB::new(tag, execution.clone());
        _ = db.state.prefetch_state(tx).await;

        let env = Box::new(Self::get_env(tx, execution, chain_id, tag).await);
        let mut evm = Self::evm(db, env);

        let tx_res = loop {
            let db = evm.db_mut();
            if db.state.needs_update() {
                db.state
                    .update_state()
                    .await
                    .map_err(|_| EvmError::Generic("failed to update state".to_string()))?;
            }

            let res = evm.transact();
            let db = evm.db_mut();
            let needs_update = db.state.needs_update();

            if res.is_ok() || !needs_update {
                break res;
            }
        };

        tx_res.map_err(|_| EvmError::Generic("evm error".to_string()))
    }

    async fn get_env(
        tx: &<Self as alloy::providers::Network>::TransactionRequest,
        execution: Arc<ExecutionClient<Self, HttpRpc<Self>>>,
        chain_id: u64,
        tag: BlockTag,
    ) -> Env {
        let mut env = Env::default();
        env.tx = Self::tx_env(tx);

        let block = execution
            .get_block(tag, false)
            .await
            .ok_or(ExecutionError::BlockNotFound(tag))
            .unwrap();
        env.block = Self::block_env(&block);

        env.cfg.chain_id = chain_id;
        env.cfg.disable_block_gas_limit = true;
        env.cfg.disable_eip3607 = true;
        env.cfg.disable_base_fee = true;

        env
    }
}

impl NetworkSpec for Ethereum {
    fn call(
        tx: &Self::TransactionRequest,
        execution: Arc<ExecutionClient<Self, HttpRpc<Self>>>,
        chain_id: u64,
        tag: BlockTag,
    ) -> impl Future<Output = Result<Bytes, EvmError>> + Send {
        async move {
            let tx = Self::call_inner(tx, execution, chain_id, tag).await?;

            match tx.result {
                ExecutionResult::Success { output, .. } => Ok(output.into_data()),
                ExecutionResult::Revert { output, .. } => {
                    Err(EvmError::Revert(Some(output.to_vec().into())))
                }
                ExecutionResult::Halt { .. } => Err(EvmError::Revert(None)),
            }
        }
    }

    fn estimate_gas(
        tx: &Self::TransactionRequest,
        execution: Arc<ExecutionClient<Self, HttpRpc<Self>>>,
        chain_id: u64,
        tag: BlockTag,
    ) -> impl Future<Output = Result<u64, EvmError>> + Send {
        async move {
            let tx = Self::call_inner(tx, execution, chain_id, tag).await?;

            match tx.result {
                ExecutionResult::Success { gas_used, .. } => Ok(gas_used),
                ExecutionResult::Revert { gas_used, .. } => Ok(gas_used),
                ExecutionResult::Halt { gas_used, .. } => Ok(gas_used),
            }
        }
    }

    fn encode_receipt(receipt: &Self::ReceiptResponse) -> Vec<u8> {
        let tx_type = receipt.transaction_type();
        let receipt = receipt.inner.as_receipt_with_bloom().unwrap();
        let logs = receipt
            .logs()
            .iter()
            .map(|l| l.inner.clone())
            .collect::<Vec<_>>();

        let consensus_receipt = Receipt {
            cumulative_gas_used: receipt.cumulative_gas_used(),
            status: receipt.status_or_post_state(),
            logs,
        };

        let rwb = ReceiptWithBloom::new(consensus_receipt, receipt.bloom());
        let encoded = alloy::rlp::encode(rwb);

        match tx_type {
            TxType::Legacy => encoded,
            _ => [vec![tx_type as u8], encoded].concat(),
        }
    }

    fn is_hash_valid(block: &Self::BlockResponse) -> bool {
        if block.header.hash_slow() != block.header.hash {
            return false;
        }

        if let Some(txs) = block.transactions.as_transactions() {
            let txs_root = calculate_transaction_root(
                &txs.iter().map(|t| t.clone().inner).collect::<Vec<_>>(),
            );
            if txs_root != block.header.transactions_root {
                return false;
            }
        }

        if let Some(withdrawals) = &block.withdrawals {
            let withdrawals_root = calculate_withdrawals_root(
                &withdrawals.iter().map(|w| w.clone()).collect::<Vec<_>>(),
            );
            if Some(withdrawals_root) != block.header.withdrawals_root {
                return false;
            }
        }

        true
    }

    fn receipt_contains(list: &[Self::ReceiptResponse], elem: &Self::ReceiptResponse) -> bool {
        for receipt in list {
            if receipt == elem {
                return true;
            }
        }

        false
    }

    fn receipt_logs(receipt: &Self::ReceiptResponse) -> Vec<Log> {
        receipt.inner.logs().to_vec()
    }
}

impl Network for Ethereum {
    type TxType = alloy::consensus::TxType;
    type TxEnvelope = alloy::consensus::TxEnvelope;
    type UnsignedTx = alloy::consensus::TypedTransaction;
    type ReceiptEnvelope = alloy::consensus::ReceiptEnvelope;
    type Header = alloy::consensus::Header;
    type TransactionRequest = alloy::rpc::types::TransactionRequest;
    type TransactionResponse = alloy::rpc::types::Transaction;
    type ReceiptResponse = alloy::rpc::types::TransactionReceipt;
    type HeaderResponse = alloy::rpc::types::Header;
    type BlockResponse = alloy::rpc::types::Block<Self::TransactionResponse, Self::HeaderResponse>;
}

impl TransactionBuilder<Ethereum> for TransactionRequest {
    fn chain_id(&self) -> Option<ChainId> {
        self.chain_id
    }

    fn set_chain_id(&mut self, chain_id: ChainId) {
        self.chain_id = Some(chain_id);
    }

    fn nonce(&self) -> Option<u64> {
        self.nonce
    }

    fn set_nonce(&mut self, nonce: u64) {
        self.nonce = Some(nonce);
    }

    fn input(&self) -> Option<&Bytes> {
        self.input.input()
    }

    fn set_input<T: Into<Bytes>>(&mut self, input: T) {
        self.input.input = Some(input.into());
    }

    fn from(&self) -> Option<Address> {
        self.from
    }

    fn set_from(&mut self, from: Address) {
        self.from = Some(from);
    }

    fn kind(&self) -> Option<TxKind> {
        self.to
    }

    fn clear_kind(&mut self) {
        self.to = None;
    }

    fn set_kind(&mut self, kind: TxKind) {
        self.to = Some(kind);
    }

    fn value(&self) -> Option<U256> {
        self.value
    }

    fn set_value(&mut self, value: U256) {
        self.value = Some(value)
    }

    fn gas_price(&self) -> Option<u128> {
        self.gas_price
    }

    fn set_gas_price(&mut self, gas_price: u128) {
        self.gas_price = Some(gas_price);
    }

    fn max_fee_per_gas(&self) -> Option<u128> {
        self.max_fee_per_gas
    }

    fn set_max_fee_per_gas(&mut self, max_fee_per_gas: u128) {
        self.max_fee_per_gas = Some(max_fee_per_gas);
    }

    fn max_priority_fee_per_gas(&self) -> Option<u128> {
        self.max_priority_fee_per_gas
    }

    fn set_max_priority_fee_per_gas(&mut self, max_priority_fee_per_gas: u128) {
        self.max_priority_fee_per_gas = Some(max_priority_fee_per_gas);
    }

    fn gas_limit(&self) -> Option<u64> {
        self.gas
    }

    fn set_gas_limit(&mut self, gas_limit: u64) {
        self.gas = Some(gas_limit);
    }

    fn access_list(&self) -> Option<&AccessList> {
        self.access_list.as_ref()
    }

    fn set_access_list(&mut self, access_list: AccessList) {
        self.access_list = Some(access_list);
    }

    fn complete_type(&self, ty: TxType) -> Result<(), Vec<&'static str>> {
        match ty {
            TxType::Legacy => self.complete_legacy(),
            TxType::Eip2930 => self.complete_2930(),
            TxType::Eip1559 => self.complete_1559(),
            TxType::Eip4844 => self.complete_4844(),
            TxType::Eip7702 => self.complete_7702(),
        }
    }

    fn can_submit(&self) -> bool {
        // value and data may be None. If they are, they will be set to default.
        // gas fields and nonce may be None, if they are, they will be populated
        // with default values by the RPC server
        self.from.is_some()
    }

    fn can_build(&self) -> bool {
        // value and data may be none. If they are, they will be set to default
        // values.

        // chain_id and from may be none.
        let common = self.gas.is_some() && self.nonce.is_some();

        let legacy = self.gas_price.is_some();
        let eip2930 = legacy
            && <TransactionRequest as TransactionBuilder<Ethereum>>::access_list(self).is_some();

        let eip1559 = self.max_fee_per_gas.is_some() && self.max_priority_fee_per_gas.is_some();

        let eip4844 = eip1559 && self.sidecar.is_some() && self.to.is_some();
        common && (legacy || eip2930 || eip1559 || eip4844)
    }

    #[doc(alias = "output_transaction_type")]
    fn output_tx_type(&self) -> TxType {
        self.preferred_type()
    }

    #[doc(alias = "output_transaction_type_checked")]
    fn output_tx_type_checked(&self) -> Option<TxType> {
        self.buildable_type()
    }

    fn prep_for_submission(&mut self) {
        self.transaction_type = Some(self.preferred_type() as u8);
        self.trim_conflicting_keys();
        self.populate_blob_hashes();
    }

    fn build_unsigned(self) -> BuildResult<TypedTransaction, Ethereum> {
        if let Err((tx_type, missing)) = self.missing_keys() {
            return Err(
                TransactionBuilderError::InvalidTransactionRequest(tx_type, missing)
                    .into_unbuilt(self),
            );
        }
        Ok(self.build_typed_tx().expect("checked by missing_keys"))
    }

    async fn build<W: NetworkWallet<Ethereum>>(
        self,
        wallet: &W,
    ) -> Result<<Ethereum as Network>::TxEnvelope, TransactionBuilderError<Ethereum>> {
        Ok(wallet.sign_request(self).await?)
    }
}
