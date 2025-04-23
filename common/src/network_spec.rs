use std::sync::Arc;

use alloy::{network::Network, primitives::Bytes, rpc::types::Log};
use async_trait::async_trait;

use crate::{
    execution_spec::ExecutionSpec,
    fork_schedule::ForkSchedule,
    types::{AccessListResultWithAccounts, BlockTag, EvmError},
};

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait NetworkSpec: Network {
    fn encode_receipt(receipt: &Self::ReceiptResponse) -> Vec<u8>;
    fn is_hash_valid(block: &Self::BlockResponse) -> bool;
    fn receipt_contains(list: &[Self::ReceiptResponse], elem: &Self::ReceiptResponse) -> bool;
    fn receipt_logs(receipt: &Self::ReceiptResponse) -> Vec<Log>;
    async fn call(
        tx: &Self::TransactionRequest,
        execution: Arc<dyn ExecutionSpec<Self>>,
        chain_id: u64,
        fork_schedule: ForkSchedule,
        tag: BlockTag,
    ) -> Result<Bytes, EvmError>;
    async fn estimate_gas(
        tx: &Self::TransactionRequest,
        execution: Arc<dyn ExecutionSpec<Self>>,
        chain_id: u64,
        fork_schedule: ForkSchedule,
        tag: BlockTag,
    ) -> Result<u64, EvmError>;
    async fn create_access_list(
        tx: &Self::TransactionRequest,
        validate_tx: bool,
        execution: Arc<dyn ExecutionSpec<Self>>,
        chain_id: u64,
        fork_schedule: ForkSchedule,
        tag: BlockTag,
    ) -> Result<AccessListResultWithAccounts, EvmError>;
}
