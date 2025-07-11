use std::{collections::HashMap, sync::Arc};

use alloy::{
    consensus::{
        proofs::{calculate_transaction_root, calculate_withdrawals_root},
        Receipt, ReceiptWithBloom, TxReceipt, TxType, TypedTransaction,
    },
    eips::{BlockId, Encodable2718},
    network::{BuildResult, Network, NetworkWallet, TransactionBuilder, TransactionBuilderError},
    primitives::{Address, Bytes, ChainId, TxKind, U256},
    rpc::types::{AccessList, Log, TransactionRequest},
};
use async_trait::async_trait;
use revm::context::result::{ExecutionResult, HaltReason};

use helios_common::{
    execution_provider::ExecutionProivder,
    fork_schedule::ForkSchedule,
    network_spec::NetworkSpec,
    types::{Account, EvmError},
};

use crate::evm::EthereumEvm;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ethereum;

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl NetworkSpec for Ethereum {
    type HaltReason = HaltReason;

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

    fn encode_transaction(tx: &Self::TransactionResponse) -> Vec<u8> {
        tx.inner.encoded_2718()
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
            let withdrawals_root =
                calculate_withdrawals_root(&withdrawals.iter().copied().collect::<Vec<_>>());
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

    async fn transact<E: ExecutionProivder<Self>>(
        tx: &Self::TransactionRequest,
        validate_tx: bool,
        execution: Arc<E>,
        chain_id: u64,
        fork_schedule: ForkSchedule,
        block_id: BlockId,
    ) -> Result<(ExecutionResult, HashMap<Address, Account>), EvmError> {
        let mut evm = EthereumEvm::new(execution, chain_id, fork_schedule, block_id);

        evm.transact_inner(tx, validate_tx).await
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

    fn take_nonce(&mut self) -> Option<u64> {
        self.nonce.take()
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
