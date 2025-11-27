use std::{collections::HashMap, fmt::Debug, sync::Arc};

use alloy::{
    eips::BlockId,
    network::Network,
    primitives::Address,
    rpc::types::{state::StateOverride, Log},
};
use async_trait::async_trait;
use revm::context::result::ExecutionResult;

use crate::{
    execution_provider::ExecutionProvider,
    fork_schedule::ForkSchedule,
    types::{Account, EvmError},
};

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait NetworkSpec: Network {
    type HaltReason: Debug + Clone + Send + Sync + Sized + 'static;

    fn encode_receipt(receipt: &Self::ReceiptResponse) -> Vec<u8>;
    fn encode_transaction(tx: &Self::TransactionResponse) -> Vec<u8>;
    fn is_hash_valid(block: &Self::BlockResponse) -> bool;
    fn receipt_contains(list: &[Self::ReceiptResponse], elem: &Self::ReceiptResponse) -> bool;
    fn receipt_logs(receipt: &Self::ReceiptResponse) -> Vec<Log>;
    async fn transact<E: ExecutionProvider<Self>>(
        tx: &Self::TransactionRequest,
        validate_tx: bool,
        execution: Arc<E>,
        chain_id: u64,
        fork_schedule: ForkSchedule,
        block_id: BlockId,
        state_overrides: Option<StateOverride>,
    ) -> Result<(ExecutionResult<Self::HaltReason>, HashMap<Address, Account>), EvmError>;
}
