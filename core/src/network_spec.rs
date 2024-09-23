use alloy::{network::Network, rpc::types::Log};
use revm::primitives::{BlockEnv, TxEnv};

use crate::types::Block;

pub trait NetworkSpec: Network {
    fn encode_receipt(receipt: &Self::ReceiptResponse) -> Vec<u8>;
    fn receipt_contains(list: &[Self::ReceiptResponse], elem: &Self::ReceiptResponse) -> bool;
    fn receipt_logs(receipt: &Self::ReceiptResponse) -> Vec<Log>;
    fn tx_env(request: &Self::TransactionRequest) -> TxEnv;
    fn block_env(block: &Block<Self::TransactionResponse>) -> BlockEnv;
}
