use alloy::{
    consensus::{
        BlockHeader, Receipt, ReceiptWithBloom, Transaction as TxTrait, TxReceipt, TxType,
    },
    primitives::{Address, Bytes, ChainId, TxKind, U256},
    rpc::types::{AccessList, Log, TransactionRequest},
};

use helios_core::network_spec::NetworkSpec;
use op_alloy_consensus::{
    OpDepositReceipt, OpDepositReceiptWithBloom, OpReceiptEnvelope, OpTxEnvelope, OpTxType,
    OpTypedTransaction,
};
use op_alloy_network::{
    BuildResult, Ethereum, Network, NetworkWallet, TransactionBuilder, TransactionBuilderError,
};
use op_alloy_rpc_types::{OpTransactionRequest, Transaction};
use revm::primitives::{BlobExcessGasAndPrice, BlockEnv, TxEnv};

#[derive(Clone, Copy, Debug)]
pub struct OpStack;

impl NetworkSpec for OpStack {
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
            _ => panic!("unreachable"),
        };

        match tx_type {
            OpTxType::Legacy => raw_encoded,
            _ => [vec![tx_type as u8], raw_encoded].concat(),
        }
    }

    fn hash_block(block: &Self::BlockResponse) -> revm::primitives::B256 {
        block.header.hash_slow()
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
        tx_env.caller =
            <OpTransactionRequest as TransactionBuilder<Self>>::from(tx).unwrap_or_default();
        tx_env.gas_limit = <OpTransactionRequest as TransactionBuilder<Self>>::gas_limit(tx)
            .map(|v| v as u64)
            .unwrap_or(u64::MAX);
        tx_env.gas_price = <OpTransactionRequest as TransactionBuilder<Self>>::gas_price(tx)
            .map(U256::from)
            .unwrap_or_default();
        tx_env.transact_to =
            <OpTransactionRequest as TransactionBuilder<Self>>::kind(tx).unwrap_or_default();
        tx_env.value =
            <OpTransactionRequest as TransactionBuilder<Self>>::value(tx).unwrap_or_default();
        tx_env.data = <OpTransactionRequest as TransactionBuilder<Self>>::input(tx)
            .unwrap_or_default()
            .clone();
        tx_env.nonce = <OpTransactionRequest as TransactionBuilder<Self>>::nonce(tx);
        tx_env.chain_id = <OpTransactionRequest as TransactionBuilder<Self>>::chain_id(tx);
        tx_env.access_list = <OpTransactionRequest as TransactionBuilder<Self>>::access_list(tx)
            .map(|v| v.to_vec())
            .unwrap_or_default();
        tx_env.gas_priority_fee =
            <OpTransactionRequest as TransactionBuilder<Self>>::max_priority_fee_per_gas(tx)
                .map(U256::from);
        tx_env.max_fee_per_blob_gas =
            <OpTransactionRequest as TransactionBuilder<Self>>::max_fee_per_gas(tx).map(U256::from);
        tx_env.blob_hashes = tx
            .clone()
            .build_typed_tx()
            .unwrap()
            .blob_versioned_hashes()
            .as_ref()
            .map(|v| v.to_vec())
            .unwrap_or_default();

        tx_env
    }

    fn block_env(block: &Self::BlockResponse) -> BlockEnv {
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
