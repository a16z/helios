use alloy::{network::Network, rpc::types::Log};
use revm::primitives::{BlockEnv, TxEnv};

use crate::fork_schedule::ForkSchedule;

pub trait NetworkSpec: Network {
    fn encode_receipt(receipt: &Self::ReceiptResponse) -> Vec<u8>;
    fn is_hash_valid(block: &Self::BlockResponse) -> bool;
    fn receipt_contains(list: &[Self::ReceiptResponse], elem: &Self::ReceiptResponse) -> bool;
    fn receipt_logs(receipt: &Self::ReceiptResponse) -> Vec<Log>;
    fn tx_env(request: &Self::TransactionRequest) -> TxEnv;
    fn block_env(block: &Self::BlockResponse, fork_schedule: &ForkSchedule) -> BlockEnv;
}
