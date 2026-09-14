use std::{collections::HashMap, sync::Arc};

use alloy::{
    consensus::{proofs::calculate_transaction_root, RlpEncodableReceipt, TxReceipt, TxType},
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
use op_alloy_consensus::{OpTxEnvelope, OpTxType, OpTypedTransaction};
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
        receipt.rlp_encode_with_bloom(&receipt_with_bloom.logs_bloom, &mut encoded);
        encoded
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
