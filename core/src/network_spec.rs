use alloy::{network::Network, rpc::types::Log};
use revm::{
    primitives::{BlockEnv, Env, TxEnv},
    Database, Evm,
};

pub trait NetworkSpec: Network {
    type EvmExt: Send + Sync;

    fn evm<DB: Database>(db: DB, env: Box<Env>) -> Evm<'static, Self::EvmExt, DB>;
    fn encode_receipt(receipt: &Self::ReceiptResponse) -> Vec<u8>;
    fn is_hash_valid(block: &Self::BlockResponse) -> bool;
    fn receipt_contains(list: &[Self::ReceiptResponse], elem: &Self::ReceiptResponse) -> bool;
    fn receipt_logs(receipt: &Self::ReceiptResponse) -> Vec<Log>;
    fn tx_env(request: &Self::TransactionRequest) -> TxEnv;
    fn block_env(block: &Self::BlockResponse) -> BlockEnv;
}
