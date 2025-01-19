use alloy::{network::Network, primitives::B256, rpc::types::Log};
use revm::primitives::{BlockEnv, TxEnv};

pub trait NetworkSpec: Network {
    fn encode_receipt(receipt: &Self::ReceiptResponse) -> Vec<u8>;
    fn hash_block(block: &Self::BlockResponse) -> B256;
    fn receipt_contains(list: &[Self::ReceiptResponse], elem: &Self::ReceiptResponse) -> bool;
    fn receipt_logs(receipt: &Self::ReceiptResponse) -> Vec<Log>;
    fn tx_env(request: &Self::TransactionRequest) -> TxEnv;
    fn block_env(block: &Self::BlockResponse) -> BlockEnv;
}
