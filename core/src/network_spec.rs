use std::{future::Future, sync::Arc};

use alloy::{network::Network, primitives::Bytes, rpc::types::Log};

use crate::{
    execution::{errors::EvmError, rpc::http_rpc::HttpRpc, ExecutionClient},
    types::BlockTag,
};

pub trait NetworkSpec: Network {
    fn call(
        tx: &Self::TransactionRequest,
        execution: Arc<ExecutionClient<Self, HttpRpc<Self>>>,
        chain_id: u64,
        tag: BlockTag,
    ) -> impl Future<Output = Result<Bytes, EvmError>> + Send;
    fn estimate_gas(
        tx: &Self::TransactionRequest,
        execution: Arc<ExecutionClient<Self, HttpRpc<Self>>>,
        chain_id: u64,
        tag: BlockTag,
    ) -> impl Future<Output = Result<u64, EvmError>> + Send;

    fn encode_receipt(receipt: &Self::ReceiptResponse) -> Vec<u8>;
    fn is_hash_valid(block: &Self::BlockResponse) -> bool;
    fn receipt_contains(list: &[Self::ReceiptResponse], elem: &Self::ReceiptResponse) -> bool;
    fn receipt_logs(receipt: &Self::ReceiptResponse) -> Vec<Log>;
}
