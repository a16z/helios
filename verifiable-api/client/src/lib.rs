use alloy::{
    eips::BlockId,
    primitives::{Address, B256, U256},
    rpc::types::Filter,
};
use async_trait::async_trait;
use eyre::Result;

use helios_common::network_spec::NetworkSpec;
use helios_verifiable_api_types::*;

pub mod http;
pub mod mock;
// re-export types
pub use helios_verifiable_api_types as types;

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait VerifiableApi<N: NetworkSpec>: Send + Clone + Sync + 'static {
    fn new(base_url: &str) -> Self
    where
        Self: Sized;
    // Methods augmented with proof
    async fn get_account(
        &self,
        address: Address,
        storage_slots: &[U256],
        block_id: Option<BlockId>,
        include_code: bool,
    ) -> Result<AccountResponse>;
    async fn get_transaction_receipt(
        &self,
        tx_hash: B256,
    ) -> Result<Option<TransactionReceiptResponse<N>>>;
    async fn get_logs(&self, filter: &Filter) -> Result<LogsResponse<N>>;
    async fn get_filter_logs(&self, filter_id: U256) -> Result<FilterLogsResponse<N>>;
    async fn get_filter_changes(&self, filter_id: U256) -> Result<FilterChangesResponse<N>>;
    async fn create_extended_access_list(
        &self,
        tx: N::TransactionRequest,
        validate_tx: bool,
        block_id: Option<BlockId>,
    ) -> Result<ExtendedAccessListResponse>;
    // Methods just for compatibility (acts as a proxy)
    async fn chain_id(&self) -> Result<ChainIdResponse>;
    async fn get_block(&self, block_id: BlockId, full_tx: bool)
        -> Result<Option<N::BlockResponse>>;
    async fn get_block_receipts(
        &self,
        block_id: BlockId,
    ) -> Result<Option<Vec<N::ReceiptResponse>>>;
    async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<SendRawTxResponse>;
    async fn new_filter(&self, filter: &Filter) -> Result<NewFilterResponse>;
    async fn new_block_filter(&self) -> Result<NewFilterResponse>;
    async fn new_pending_transaction_filter(&self) -> Result<NewFilterResponse>;
    async fn uninstall_filter(&self, filter_id: U256) -> Result<UninstallFilterResponse>;
}
