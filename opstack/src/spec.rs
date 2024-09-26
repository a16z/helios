use alloy::{
    consensus::{
        BlobTransactionSidecar, Receipt, ReceiptWithBloom, TxReceipt, TxType, TypedTransaction,
    },
    network::Network,
    primitives::{Address, Bytes, ChainId, TxKind, U256},
    rpc::types::{AccessList, Log, TransactionRequest},
};
use helios_core::{network_spec::NetworkSpec, types::Block};
use op_alloy_consensus::{
    OpDepositReceipt, OpDepositReceiptWithBloom, OpReceiptEnvelope, OpTxType,
};
use op_alloy_network::{
    BuildResult, Ethereum, NetworkWallet, TransactionBuilder, TransactionBuilderError,
};
use revm::primitives::{BlobExcessGasAndPrice, BlockEnv, TxEnv};

#[derive(Clone, Copy, Debug)]
pub struct Optimism;

impl NetworkSpec for Optimism {
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
            | OpReceiptEnvelope::Eip4844(inner) => {
                let r = Receipt {
                    status: *inner.status_or_post_state(),
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
            _ => panic!("unreachable"),
        };

        match tx_type {
            OpTxType::Legacy => raw_encoded,
            _ => [vec![tx_type as u8], raw_encoded].concat(),
        }
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

    fn tx_env(tx: &Self::TransactionRequest) -> TxEnv {
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

    fn block_env(block: &Block<Self::TransactionResponse>) -> BlockEnv {
        let mut block_env = BlockEnv::default();
        block_env.number = block.number.to();
        block_env.coinbase = block.miner;
        block_env.timestamp = block.timestamp.to();
        block_env.gas_limit = block.gas_limit.to();
        block_env.basefee = block.base_fee_per_gas;
        block_env.difficulty = block.difficulty;
        block_env.prevrandao = Some(block.mix_hash);
        block_env.blob_excess_gas_and_price = block
            .excess_blob_gas
            .map(|v| BlobExcessGasAndPrice::new(v.to()));

        block_env
    }
}

impl Network for Optimism {
    type TxType = op_alloy_consensus::OpTxType;
    type TxEnvelope = alloy::consensus::TxEnvelope;
    type UnsignedTx = alloy::consensus::TypedTransaction;
    type ReceiptEnvelope = op_alloy_consensus::OpReceiptEnvelope;
    type Header = alloy::consensus::Header;
    type TransactionRequest = TransactionRequest;
    type TransactionResponse = alloy::rpc::types::Transaction;
    type ReceiptResponse = op_alloy_rpc_types::OpTransactionReceipt;
    type HeaderResponse = alloy::rpc::types::Header;
}

impl TransactionBuilder<Optimism> for TransactionRequest {
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

    fn max_fee_per_blob_gas(&self) -> Option<u128> {
        self.max_fee_per_blob_gas
    }

    fn set_max_fee_per_blob_gas(&mut self, max_fee_per_blob_gas: u128) {
        self.max_fee_per_blob_gas = Some(max_fee_per_blob_gas)
    }

    fn gas_limit(&self) -> Option<u128> {
        self.gas
    }

    fn set_gas_limit(&mut self, gas_limit: u128) {
        self.gas = Some(gas_limit);
    }

    fn access_list(&self) -> Option<&AccessList> {
        self.access_list.as_ref()
    }

    fn set_access_list(&mut self, access_list: AccessList) {
        self.access_list = Some(access_list);
    }

    fn blob_sidecar(&self) -> Option<&BlobTransactionSidecar> {
        self.sidecar.as_ref()
    }

    fn set_blob_sidecar(&mut self, sidecar: BlobTransactionSidecar) {
        TransactionBuilder::<Ethereum>::set_blob_sidecar(self, sidecar)
    }

    fn complete_type(&self, ty: OpTxType) -> Result<(), Vec<&'static str>> {
        match ty {
            OpTxType::Deposit => Err(vec!["not implemented for deposit tx"]),
            _ => {
                let ty = TxType::try_from(ty as u8).unwrap();
                TransactionBuilder::<Ethereum>::complete_type(self, ty)
            }
        }
    }

    fn can_submit(&self) -> bool {
        TransactionBuilder::<Ethereum>::can_submit(self)
    }

    fn can_build(&self) -> bool {
        TransactionBuilder::<Ethereum>::can_build(self)
    }

    #[doc(alias = "output_transaction_type")]
    fn output_tx_type(&self) -> OpTxType {
        OpTxType::try_from(self.preferred_type() as u8).unwrap()
    }

    #[doc(alias = "output_transaction_type_checked")]
    fn output_tx_type_checked(&self) -> Option<OpTxType> {
        self.buildable_type()
            .map(|tx_ty| OpTxType::try_from(tx_ty as u8).unwrap())
    }

    fn prep_for_submission(&mut self) {
        self.transaction_type = Some(self.preferred_type() as u8);
        self.trim_conflicting_keys();
        self.populate_blob_hashes();
    }

    fn build_unsigned(self) -> BuildResult<TypedTransaction, Optimism> {
        if let Err((tx_type, missing)) = self.missing_keys() {
            let tx_type = OpTxType::try_from(tx_type as u8).unwrap();
            return Err(
                TransactionBuilderError::InvalidTransactionRequest(tx_type, missing)
                    .into_unbuilt(self),
            );
        }
        Ok(self.build_typed_tx().expect("checked by missing_keys"))
    }

    async fn build<W: NetworkWallet<Optimism>>(
        self,
        wallet: &W,
    ) -> Result<<Optimism as Network>::TxEnvelope, TransactionBuilderError<Optimism>> {
        Ok(wallet.sign_request(self).await?)
    }
}
