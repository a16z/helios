use alloy::eips::BlockNumberOrTag;
use alloy::primitives::{Address, B256, U256};
use alloy::rpc::types::{
    AccessList, BlockId, BlockTransactionsKind, EIP1186AccountProofResponse, FeeHistory, Filter,
    FilterChanges, Log,
};
use async_trait::async_trait;
use eyre::Result;

use helios_common::network_spec::NetworkSpec;

pub mod http_rpc;
pub mod mock_rpc;

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait ExecutionRpc<N: NetworkSpec>: Send + Clone + Sync + 'static {
    fn new(rpc: &str) -> Result<Self>
    where
        Self: Sized;

    async fn get_proof(
        &self,
        address: Address,
        slots: &[B256],
        block: BlockId,
    ) -> Result<EIP1186AccountProofResponse>;

    async fn create_access_list(
        &self,
        tx: &N::TransactionRequest,
        block: BlockId,
    ) -> Result<AccessList>;

    async fn get_code(&self, address: Address, block: u64) -> Result<Vec<u8>>;
    async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<B256>;
    async fn get_transaction_receipt(&self, tx_hash: B256) -> Result<Option<N::ReceiptResponse>>;
    async fn get_block_receipts(&self, block: BlockId) -> Result<Option<Vec<N::ReceiptResponse>>>;
    async fn get_transaction(&self, tx_hash: B256) -> Result<Option<N::TransactionResponse>>;
    async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>>;
    async fn get_filter_changes(&self, filter_id: U256) -> Result<FilterChanges>;
    async fn get_filter_logs(&self, filter_id: U256) -> Result<Vec<Log>>;
    async fn uninstall_filter(&self, filter_id: U256) -> Result<bool>;
    async fn new_filter(&self, filter: &Filter) -> Result<U256>;
    async fn new_block_filter(&self) -> Result<U256>;
    async fn new_pending_transaction_filter(&self) -> Result<U256>;
    async fn chain_id(&self) -> Result<u64>;
    async fn get_block(&self, hash: B256) -> Result<N::BlockResponse>;
    async fn get_block_by_number(
        &self,
        block: BlockNumberOrTag,
        txs_kind: BlockTransactionsKind,
    ) -> Result<Option<N::BlockResponse>>;

    async fn get_fee_history(
        &self,
        block_count: u64,
        last_block: u64,
        reward_percentiles: &[f64],
    ) -> Result<FeeHistory>;
}
