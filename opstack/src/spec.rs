use std::{collections::HashMap, sync::Arc};

use alloy::{
    consensus::{proofs::calculate_transaction_root, Receipt, ReceiptWithBloom, TxReceipt, TxType},
    eips::{BlockId, Encodable2718},
    primitives::{Address, Bytes, ChainId, TxKind, U256},
    rpc::types::{AccessList, Log, TransactionRequest},
};

use async_trait::async_trait;
use helios_common::{
    execution_provider::ExecutionProivder,
    fork_schedule::ForkSchedule,
    network_spec::NetworkSpec,
    types::{Account, EvmError},
};
use op_alloy_consensus::{
    OpDepositReceipt, OpDepositReceiptWithBloom, OpReceiptEnvelope, OpTxEnvelope, OpTxType,
    OpTypedTransaction,
};
use op_alloy_network::{
    BuildResult, Ethereum, Network, NetworkWallet, TransactionBuilder, TransactionBuilderError,
};
use op_alloy_rpc_types::{OpTransactionRequest, Transaction};
use op_revm::OpHaltReason;
use revm::context::result::ExecutionResult;

use crate::evm::OpStackEvm;

#[derive(Clone, Copy, Debug)]
pub struct OpStack;

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl NetworkSpec for OpStack {
    type HaltReason = OpHaltReason;

    fn encode_receipt(receipt: &Self::ReceiptResponse) -> Vec<u8> {
        let receipt = &receipt.inner.inner;
        let bloom = receipt.bloom();
        let tx_type = receipt.tx_type();
        let logs = receipt
            .logs()
            .iter()
            .map(|l| l.inner.clone())
            .collect::<Vec<_>>();

        let raw_encoded = match receipt {
            OpReceiptEnvelope::Legacy(inner)
            | OpReceiptEnvelope::Eip2930(inner)
            | OpReceiptEnvelope::Eip1559(inner)
            | OpReceiptEnvelope::Eip7702(inner) => {
                let r = Receipt {
                    status: inner.status_or_post_state(),
                    cumulative_gas_used: inner.cumulative_gas_used(),
                    logs,
                };
                let rwb = ReceiptWithBloom::new(r, bloom);
                alloy::rlp::encode(rwb)
            }
            OpReceiptEnvelope::Deposit(inner) => {
                let r = Receipt {
                    status: inner.receipt.inner.status,
                    cumulative_gas_used: inner.receipt.inner.cumulative_gas_used,
                    logs,
                };

                let r = OpDepositReceipt {
                    inner: r,
                    deposit_nonce: inner.receipt.deposit_nonce,
                    deposit_receipt_version: inner.receipt.deposit_receipt_version,
                };

                let rwb = OpDepositReceiptWithBloom::new(r, bloom);
                alloy::rlp::encode(rwb)
            }
        };

        match tx_type {
            OpTxType::Legacy => raw_encoded,
            _ => [vec![tx_type as u8], raw_encoded].concat(),
        }
    }

    fn encode_transaction(tx: &Self::TransactionResponse) -> Vec<u8> {
        tx.inner.inner.encoded_2718()
    }

    fn is_hash_valid(block: &Self::BlockResponse) -> bool {
        if block.header.hash_slow() != block.header.hash {
            return false;
        }

        if let Some(txs) = block.transactions.as_transactions() {
            let txs_root = calculate_transaction_root(
                &txs.iter()
                    .map(|t| t.clone().inner.inner)
                    .collect::<Vec<_>>(),
            );
            if txs_root != block.header.transactions_root {
                return false;
            }
        }

        if let Some(withdrawals) = &block.withdrawals {
            if !withdrawals.0.is_empty() {
                return false;
            }
            // TODO: handle L2ToL1MessagePasser storage root check
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
        receipt.inner.inner.logs().to_vec()
    }

    async fn transact<E: ExecutionProivder<Self>>(
        tx: &Self::TransactionRequest,
        validate_tx: bool,
        execution: Arc<E>,
        chain_id: u64,
        fork_schedule: ForkSchedule,
        block_id: BlockId,
    ) -> Result<(ExecutionResult<Self::HaltReason>, HashMap<Address, Account>), EvmError> {
        let mut evm = OpStackEvm::new(execution, chain_id, fork_schedule, block_id);

        evm.transact_inner(tx, validate_tx).await
    }
}

impl Network for OpStack {
    type TxType = op_alloy_consensus::OpTxType;
    type TxEnvelope = OpTxEnvelope;
    type UnsignedTx = OpTypedTransaction;
    type ReceiptEnvelope = op_alloy_consensus::OpReceiptEnvelope;
    type Header = alloy::consensus::Header;
    type TransactionRequest = OpTransactionRequest;
    type TransactionResponse = Transaction;
    type ReceiptResponse = op_alloy_rpc_types::OpTransactionReceipt;
    type HeaderResponse = alloy::rpc::types::Header;
    type BlockResponse = alloy::rpc::types::Block<Self::TransactionResponse, Self::HeaderResponse>;
}

impl TransactionBuilder<OpStack> for OpTransactionRequest {
    fn chain_id(&self) -> Option<ChainId> {
        <TransactionRequest as TransactionBuilder<Ethereum>>::chain_id(self.as_ref())
    }

    fn set_chain_id(&mut self, chain_id: ChainId) {
        <TransactionRequest as TransactionBuilder<Ethereum>>::set_chain_id(self.as_mut(), chain_id);
    }

    fn nonce(&self) -> Option<u64> {
        <TransactionRequest as TransactionBuilder<Ethereum>>::nonce(self.as_ref())
    }

    fn set_nonce(&mut self, nonce: u64) {
        <TransactionRequest as TransactionBuilder<Ethereum>>::set_nonce(self.as_mut(), nonce);
    }

    fn take_nonce(&mut self) -> Option<u64> {
        <TransactionRequest as TransactionBuilder<Ethereum>>::take_nonce(self.as_mut())
    }

    fn input(&self) -> Option<&Bytes> {
        <TransactionRequest as TransactionBuilder<Ethereum>>::input(self.as_ref())
    }

    fn set_input<T: Into<Bytes>>(&mut self, input: T) {
        <TransactionRequest as TransactionBuilder<Ethereum>>::set_input(self.as_mut(), input);
    }

    fn from(&self) -> Option<Address> {
        <TransactionRequest as TransactionBuilder<Ethereum>>::from(self.as_ref())
    }

    fn set_from(&mut self, from: Address) {
        <TransactionRequest as TransactionBuilder<Ethereum>>::set_from(self.as_mut(), from);
    }

    fn kind(&self) -> Option<TxKind> {
        <TransactionRequest as TransactionBuilder<Ethereum>>::kind(self.as_ref())
    }

    fn clear_kind(&mut self) {
        <TransactionRequest as TransactionBuilder<Ethereum>>::clear_kind(self.as_mut());
    }

    fn set_kind(&mut self, kind: TxKind) {
        <TransactionRequest as TransactionBuilder<Ethereum>>::set_kind(self.as_mut(), kind);
    }

    fn value(&self) -> Option<U256> {
        <TransactionRequest as TransactionBuilder<Ethereum>>::value(self.as_ref())
    }

    fn set_value(&mut self, value: U256) {
        <TransactionRequest as TransactionBuilder<Ethereum>>::set_value(self.as_mut(), value);
    }

    fn gas_price(&self) -> Option<u128> {
        <TransactionRequest as TransactionBuilder<Ethereum>>::gas_price(self.as_ref())
    }

    fn set_gas_price(&mut self, gas_price: u128) {
        <TransactionRequest as TransactionBuilder<Ethereum>>::set_gas_price(
            self.as_mut(),
            gas_price,
        );
    }

    fn max_fee_per_gas(&self) -> Option<u128> {
        <TransactionRequest as TransactionBuilder<Ethereum>>::max_fee_per_gas(self.as_ref())
    }

    fn set_max_fee_per_gas(&mut self, max_fee_per_gas: u128) {
        <TransactionRequest as TransactionBuilder<Ethereum>>::set_max_fee_per_gas(
            self.as_mut(),
            max_fee_per_gas,
        );
    }

    fn max_priority_fee_per_gas(&self) -> Option<u128> {
        <TransactionRequest as TransactionBuilder<Ethereum>>::max_priority_fee_per_gas(
            self.as_ref(),
        )
    }

    fn set_max_priority_fee_per_gas(&mut self, max_priority_fee_per_gas: u128) {
        <TransactionRequest as TransactionBuilder<Ethereum>>::set_max_priority_fee_per_gas(
            self.as_mut(),
            max_priority_fee_per_gas,
        );
    }

    fn gas_limit(&self) -> Option<u64> {
        <TransactionRequest as TransactionBuilder<Ethereum>>::gas_limit(self.as_ref())
    }

    fn set_gas_limit(&mut self, gas_limit: u64) {
        <TransactionRequest as TransactionBuilder<Ethereum>>::set_gas_limit(
            self.as_mut(),
            gas_limit,
        );
    }

    fn access_list(&self) -> Option<&AccessList> {
        <TransactionRequest as TransactionBuilder<Ethereum>>::access_list(self.as_ref())
    }

    fn set_access_list(&mut self, access_list: AccessList) {
        <TransactionRequest as TransactionBuilder<Ethereum>>::set_access_list(
            self.as_mut(),
            access_list,
        );
    }

    fn complete_type(&self, ty: OpTxType) -> Result<(), Vec<&'static str>> {
        match ty {
            OpTxType::Deposit => Err(vec!["not implemented for deposit tx"]),
            _ => {
                let ty = TxType::try_from(ty as u8).unwrap();
                <TransactionRequest as TransactionBuilder<Ethereum>>::complete_type(
                    self.as_ref(),
                    ty,
                )
            }
        }
    }

    fn can_submit(&self) -> bool {
        <TransactionRequest as TransactionBuilder<Ethereum>>::can_submit(self.as_ref())
    }

    fn can_build(&self) -> bool {
        <TransactionRequest as TransactionBuilder<Ethereum>>::can_build(self.as_ref())
    }

    #[doc(alias = "output_transaction_type")]
    fn output_tx_type(&self) -> OpTxType {
        match self.as_ref().preferred_type() {
            TxType::Eip1559 | TxType::Eip4844 => OpTxType::Eip1559,
            TxType::Eip2930 => OpTxType::Eip2930,
            TxType::Eip7702 => OpTxType::Eip7702,
            TxType::Legacy => OpTxType::Legacy,
        }
    }

    #[doc(alias = "output_transaction_type_checked")]
    fn output_tx_type_checked(&self) -> Option<OpTxType> {
        self.as_ref().buildable_type().map(|tx_ty| match tx_ty {
            TxType::Eip1559 | TxType::Eip4844 => OpTxType::Eip1559,
            TxType::Eip2930 => OpTxType::Eip2930,
            TxType::Eip7702 => OpTxType::Eip7702,
            TxType::Legacy => OpTxType::Legacy,
        })
    }

    fn prep_for_submission(&mut self) {
        <TransactionRequest as TransactionBuilder<Ethereum>>::prep_for_submission(self.as_mut());
    }

    fn build_unsigned(self) -> BuildResult<OpTypedTransaction, OpStack> {
        if let Err((tx_type, missing)) = self.as_ref().missing_keys() {
            let tx_type = OpTxType::try_from(tx_type as u8).unwrap();
            return Err(
                TransactionBuilderError::InvalidTransactionRequest(tx_type, missing)
                    .into_unbuilt(self),
            );
        }
        Ok(self.build_typed_tx().expect("checked by missing_keys"))
    }

    async fn build<W: NetworkWallet<OpStack>>(
        self,
        wallet: &W,
    ) -> Result<<OpStack as Network>::TxEnvelope, TransactionBuilderError<OpStack>> {
        Ok(wallet.sign_request(self).await?)
    }
}
