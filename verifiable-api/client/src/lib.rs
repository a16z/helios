use alloy::{
    eips::BlockId,
    primitives::{Address, B256, U256},
    rpc::types::Filter,
};
use async_trait::async_trait;
use eyre::Result;
use url::Url;

use helios_common::network_spec::NetworkSpec;
use helios_verifiable_api_types::*;

pub mod http;
pub mod mock;
// re-export types
pub use helios_verifiable_api_types as types;

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait VerifiableApi<N: NetworkSpec>: Send + Clone + Sync + Sized + 'static {
    fn new(base_url: &Url) -> Self
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
    async fn get_transaction(&self, tx_hash: B256) -> Result<Option<TransactionResponse<N>>>;
    async fn get_transaction_by_location(
        &self,
        block_id: BlockId,
        index: u64,
    ) -> Result<Option<TransactionResponse<N>>>;
    async fn get_logs(&self, filter: &Filter) -> Result<LogsResponse<N>>;
    async fn get_execution_hint(
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
}
