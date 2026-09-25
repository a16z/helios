use alloy::{
    consensus::{transaction::SignerRecoverable, Transaction as _},
    primitives::keccak256,
};
use std::{collections::HashMap, sync::Arc};

use alloy::{
    consensus::{proofs::calculate_transaction_root, Eip2718EncodableReceipt, TxReceipt, TxType},
    eips::{BlockId, Encodable2718},
    primitives::Address,
    rpc::types::{state::StateOverride, Log},
};

use async_trait::async_trait;
use helios_common::{
    execution_provider::ExecutionProvider,
    fork_schedule::ForkSchedule,
    network_spec::NetworkSpec,
    types::{Account, EvmError},
};
use op_alloy_consensus::{OpTxEnvelope, OpTxReceipt, OpTxType, OpTypedTransaction};
use op_alloy_network::{
    BuildResult, Ethereum, Network, NetworkTransactionBuilder, NetworkWallet,
    TransactionBuilderError,
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
        let receipt_with_bloom = &receipt.inner.inner;
        let receipt = receipt_with_bloom.receipt.clone().map_logs(|log| log.inner);
        let mut encoded = Vec::new();
        receipt.eip2718_encode_with_bloom(&receipt_with_bloom.logs_bloom, &mut encoded);
        encoded
    }

    fn encode_transaction(tx: &Self::TransactionResponse) -> Vec<u8> {
        tx.inner.inner.encoded_2718()
    }

    fn is_hash_valid(block: &Self::BlockResponse) -> bool {
        if block.header.hash_slow() != block.header.hash {
            return false;
        }

        let Some(txs) = block.transactions.as_transactions() else {
            return false;
        };
        if calculate_transaction_root(
            &txs.iter()
                .map(|t| t.clone().inner.inner)
                .collect::<Vec<_>>(),
        ) != block.header.transactions_root
        {
            return false;
        }

        if let Some(withdrawals) = &block.withdrawals {
            if !withdrawals.0.is_empty() {
                return false;
            }
            // TODO: handle L2ToL1MessagePasser storage root check
        }

        block.uncles.is_empty()
    }

    fn validate_block(block: &mut Self::BlockResponse, full_tx: bool) -> bool {
        if !Self::is_hash_valid(block) {
            return false;
        }
        let alloy::rpc::types::BlockTransactions::Full(txs) = &mut block.transactions else {
            return false;
        };
        if full_tx {
            for (index, tx) in txs.iter_mut().enumerate() {
                if keccak256(tx.inner.inner.encoded_2718()) != *tx.inner.inner.tx_hash()
                    || tx.inner.inner.inner().recover_signer().ok() != Some(tx.inner.inner.signer())
                    || tx.inner.block_hash != Some(block.header.hash)
                    || tx.inner.block_number != Some(block.header.number)
                    || tx.inner.transaction_index != Some(index as u64)
                {
                    return false;
                }
                tx.inner.effective_gas_price =
                    Some(tx.effective_gas_price(block.header.base_fee_per_gas));
                tx.inner.block_timestamp = Some(block.header.timestamp);
                // These belong to the receipt and are not part of the transaction trie.
                tx.deposit_nonce = None;
                tx.deposit_receipt_version = None;
            }
        } else {
            // Cached RPC hashes are not committed by the transaction trie. Derive
            // the only transaction field exposed by this response from signed bytes.
            block.transactions = alloy::rpc::types::BlockTransactions::Hashes(
                txs.iter()
                    .map(|tx| keccak256(Self::encode_transaction(tx)))
                    .collect(),
            );
        }
        block.header.total_difficulty = None;
        block.header.size = None;
        block.uncles.is_empty()
    }

    fn receipt_contains(list: &[Self::ReceiptResponse], elem: &Self::ReceiptResponse) -> bool {
        for receipt in list {
            if receipt == elem {
                return true;
            }
        }

        false
    }

    fn sanitize_receipt(receipt: &mut Self::ReceiptResponse) {
        // L1/operator fee fields and gas refunds are not encoded in the OP receipt trie.
        receipt.l1_block_info = Default::default();
        receipt.op_gas_refund = None;
    }

    fn receipt_logs(receipt: &Self::ReceiptResponse) -> Vec<Log> {
        receipt.inner.inner.logs().to_vec()
    }

    fn receipt_metadata_valid(
        receipt: &Self::ReceiptResponse,
        tx: &Self::TransactionResponse,
        block: &Self::BlockResponse,
        _forks: &ForkSchedule,
    ) -> bool {
        use alloy::consensus::Transaction;
        let nonce = receipt
            .inner
            .inner
            .receipt
            .deposit_nonce()
            .unwrap_or_else(|| tx.nonce());
        let contract_address = tx
            .is_create()
            .then(|| tx.inner.inner.signer().create(nonce));
        receipt.inner.contract_address == contract_address
            && receipt.inner.effective_gas_price
                == tx.effective_gas_price(block.header.base_fee_per_gas)
            && receipt.inner.blob_gas_used.is_none()
            && receipt.inner.blob_gas_price.is_none()
    }

    async fn transact<E: ExecutionProvider<Self>>(
        tx: &Self::TransactionRequest,
        validate_tx: bool,
        execution: Arc<E>,
        chain_id: u64,
        fork_schedule: ForkSchedule,
        block_id: BlockId,
        state_overrides: Option<StateOverride>,
    ) -> Result<(ExecutionResult<Self::HaltReason>, HashMap<Address, Account>), EvmError> {
        let mut evm = OpStackEvm::new(execution, chain_id, fork_schedule, block_id);

        evm.transact_inner(tx, validate_tx, state_overrides).await
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

impl NetworkTransactionBuilder<OpStack> for OpTransactionRequest {
    fn complete_type(&self, ty: OpTxType) -> Result<(), Vec<&'static str>> {
        match ty {
            OpTxType::Deposit => Err(vec!["not implemented for deposit tx"]),
            OpTxType::PostExec => Err(vec!["not implemented for post-exec tx"]),
            _ => {
                let ty = TxType::try_from(ty as u8).unwrap();
                NetworkTransactionBuilder::<Ethereum>::complete_type(self.as_ref(), ty)
            }
        }
    }

    fn can_submit(&self) -> bool {
        NetworkTransactionBuilder::<Ethereum>::can_submit(self.as_ref())
    }

    fn can_build(&self) -> bool {
        NetworkTransactionBuilder::<Ethereum>::can_build(self.as_ref())
    }

    #[doc(alias = "output_transaction_type")]
    fn output_tx_type(&self) -> OpTxType {
        match NetworkTransactionBuilder::<Ethereum>::output_tx_type(self.as_ref()) {
            TxType::Eip1559 | TxType::Eip4844 => OpTxType::Eip1559,
            TxType::Eip2930 => OpTxType::Eip2930,
            TxType::Eip7702 => OpTxType::Eip7702,
            TxType::Legacy => OpTxType::Legacy,
        }
    }

    #[doc(alias = "output_transaction_type_checked")]
    fn output_tx_type_checked(&self) -> Option<OpTxType> {
        NetworkTransactionBuilder::<Ethereum>::output_tx_type_checked(self.as_ref()).map(|tx_ty| {
            match tx_ty {
                TxType::Eip1559 | TxType::Eip4844 => OpTxType::Eip1559,
                TxType::Eip2930 => OpTxType::Eip2930,
                TxType::Eip7702 => OpTxType::Eip7702,
                TxType::Legacy => OpTxType::Legacy,
            }
        })
    }

    fn prep_for_submission(&mut self) {
        NetworkTransactionBuilder::<Ethereum>::prep_for_submission(self.as_mut());
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
