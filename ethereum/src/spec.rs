use alloy::{
    consensus::{
        proofs::{calculate_transaction_root, calculate_withdrawals_root},
        BlockHeader, Receipt, ReceiptWithBloom, TxReceipt, TxType, TypedTransaction,
    },
    eips::Encodable2718,
    network::{BuildResult, Network, NetworkWallet, TransactionBuilder, TransactionBuilderError},
    primitives::{Address, Bytes, ChainId, TxKind, U256},
    rlp,
    rpc::types::{AccessList, Log, TransactionRequest},
};

use helios_common::{fork_schedule::ForkSchedule, network_spec::NetworkSpec};
use revm::{
    context::{BlockEnv, TxEnv},
    context_interface::block::BlobExcessGasAndPrice,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ethereum;

impl NetworkSpec for Ethereum {
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

    fn tx_env(tx: &Self::TransactionRequest) -> TxEnv {
        TxEnv {
            tx_type: tx.transaction_type.unwrap_or_default(),
            caller: tx.from.unwrap_or_default(),
            gas_limit: <TransactionRequest as TransactionBuilder<Self>>::gas_limit(tx)
                .unwrap_or(u64::MAX),
            gas_price: <TransactionRequest as TransactionBuilder<Self>>::gas_price(tx)
                .unwrap_or_default(),
            kind: tx.to.unwrap_or_default(),
            value: tx.value.unwrap_or_default(),
            data: <TransactionRequest as TransactionBuilder<Self>>::input(tx)
                .unwrap_or_default()
                .clone(),
            nonce: <TransactionRequest as TransactionBuilder<Self>>::nonce(tx).unwrap_or_default(),
            chain_id: <TransactionRequest as TransactionBuilder<Self>>::chain_id(tx),
            access_list: <TransactionRequest as TransactionBuilder<Self>>::access_list(tx)
                .cloned()
                .unwrap_or_default(),
            gas_priority_fee:
                <TransactionRequest as TransactionBuilder<Self>>::max_priority_fee_per_gas(tx),
            max_fee_per_blob_gas: tx.max_fee_per_blob_gas.unwrap_or_default(),
            blob_hashes: tx
                .blob_versioned_hashes
                .as_ref()
                .map(|v| v.to_vec())
                .unwrap_or_default(),
            authorization_list: vec![],
        }
    }

    fn block_env(block: &Self::BlockResponse, fork_schedule: &ForkSchedule) -> BlockEnv {
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
