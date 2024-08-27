use alloy::{
    consensus::{Receipt, ReceiptWithBloom, TxReceipt, TxType},
    network::{Ethereum, Network, TransactionBuilder},
    rpc::types::{Log, Transaction, TransactionReceipt, TransactionRequest},
};
use revm::primitives::{BlobExcessGasAndPrice, BlockEnv, TxEnv, U256};

use crate::types::Block;

pub trait NetworkSpec: Network {
    fn encode_receipt(receipt: &Self::ReceiptResponse) -> Vec<u8>;
    fn receipt_contains(list: &[Self::ReceiptResponse], elem: &Self::ReceiptResponse) -> bool;
    fn receipt_logs(receipt: &Self::ReceiptResponse) -> Vec<Log>;
    fn tx_env(request: &Self::TransactionRequest) -> TxEnv;
    fn block_env(block: &Block<Self::TransactionResponse>) -> BlockEnv;
}

impl NetworkSpec for Ethereum {
    fn encode_receipt(receipt: &TransactionReceipt) -> Vec<u8> {
        let tx_type = receipt.transaction_type();
        let receipt = receipt.inner.as_receipt_with_bloom().unwrap();
        let logs = receipt
            .logs()
            .iter()
            .map(|l| l.inner.clone())
            .collect::<Vec<_>>();

        let consensus_receipt = Receipt {
            cumulative_gas_used: receipt.cumulative_gas_used(),
            status: *receipt.status_or_post_state(),
            logs,
        };

        let rwb = ReceiptWithBloom::new(consensus_receipt, receipt.bloom());
        let encoded = alloy::rlp::encode(rwb);

        match tx_type {
            TxType::Legacy => encoded,
            _ => [vec![tx_type as u8], encoded].concat(),
        }
    }

    fn receipt_contains(list: &[TransactionReceipt], elem: &TransactionReceipt) -> bool {
        for receipt in list {
            if receipt == elem {
                return true;
            }
        }

        false
    }

    fn receipt_logs(receipt: &TransactionReceipt) -> Vec<Log> {
        receipt.inner.logs().to_vec()
    }

    fn tx_env(tx: &TransactionRequest) -> TxEnv {
        let mut tx_env = TxEnv::default();
        tx_env.caller = tx.from.unwrap_or_default();
        tx_env.gas_limit = tx.gas_limit().map(|v| v as u64).unwrap_or(u64::MAX);
        tx_env.gas_price = tx.gas_price().map(U256::from).unwrap_or_default();
        tx_env.transact_to = tx.to.unwrap_or_default();
        tx_env.value = tx.value.unwrap_or_default();
        tx_env.data = tx.input().unwrap_or_default().clone();
        tx_env.nonce = tx.nonce();
        tx_env.chain_id = tx.chain_id();
        tx_env.access_list = tx.access_list().map(|v| v.to_vec()).unwrap_or_default();
        tx_env.gas_priority_fee = tx.max_priority_fee_per_gas().map(U256::from);
        tx_env.max_fee_per_blob_gas = tx.max_fee_per_gas().map(U256::from);
        tx_env.blob_hashes = tx
            .blob_versioned_hashes
            .as_ref()
            .map(|v| v.to_vec())
            .unwrap_or_default();

        tx_env
    }

    fn block_env(block: &Block<Transaction>) -> BlockEnv {
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
