use alloy::{
    eips::BlockId,
    primitives::{Address, Bytes, B256, U256},
    rpc::types::{
        AccessListResult, EIP1186AccountProofResponse, Filter, FilterChanges, Log, SyncStatus,
    },
};
use async_trait::async_trait;
use eyre::Result;

use helios_common::{
    network_spec::NetworkSpec,
    types::{SubEventRx, SubscriptionType},
};

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait HeliosApi<N: NetworkSpec>: Send + Sync + 'static {
    // node management
    async fn wait_synced(&self) -> Result<()>;
    async fn shutdown(&self);
    // state fetch
    async fn get_balance(&self, address: Address, block_id: BlockId) -> Result<U256>;
    async fn get_nonce(&self, address: Address, block_id: BlockId) -> Result<u64>;
    async fn get_code(&self, address: Address, block_id: BlockId) -> Result<Bytes>;
    async fn get_storage_at(&self, address: Address, slot: U256, block_id: BlockId)
        -> Result<B256>;
    async fn get_proof(
        &self,
        address: Address,
        slots: &[B256],
        block_id: BlockId,
    ) -> Result<EIP1186AccountProofResponse>;
    // gas accounting
    async fn get_gas_price(&self) -> Result<U256>;
    async fn get_priority_fee(&self) -> Result<U256>;
    async fn get_blob_base_fee(&self) -> Result<U256>;
    // block info
    async fn get_block_number(&self) -> Result<U256>;
    async fn get_block_transaction_count(&self, block_id: BlockId) -> Result<Option<u64>>;
    async fn get_block(&self, block_id: BlockId, full_tx: bool)
        -> Result<Option<N::BlockResponse>>;
    // transactions
    async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<B256>;
    async fn get_transaction(&self, tx_hash: B256) -> Result<Option<N::TransactionResponse>>;
    async fn get_transaction_by_block_and_index(
        &self,
        block_id: BlockId,
        index: u64,
    ) -> Result<Option<N::TransactionResponse>>;
    // receipts
    async fn get_transaction_receipt(&self, tx_hash: B256) -> Result<Option<N::ReceiptResponse>>;
    async fn get_block_receipts(
        &self,
        block_id: BlockId,
    ) -> Result<Option<Vec<N::ReceiptResponse>>>;
    // evm
    async fn call(&self, tx: &N::TransactionRequest, block_id: BlockId) -> Result<Bytes>;
    async fn estimate_gas(&self, tx: &N::TransactionRequest, block_id: BlockId) -> Result<u64>;
    async fn create_access_list(
        &self,
        tx: &N::TransactionRequest,
        block_id: BlockId,
    ) -> Result<AccessListResult>;
    // logs
    async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>>;
    // filters and subscriptions
    async fn subscribe(&self, sub_type: SubscriptionType) -> Result<SubEventRx<N>>;
    async fn get_filter_logs(&self, filter_id: U256) -> Result<Vec<Log>>;
    async fn get_filter_changes(&self, filter_id: U256) -> Result<FilterChanges>;
    async fn uninstall_filter(&self, filter_id: U256) -> Result<bool>;
    async fn new_filter(&self, filter: &Filter) -> Result<U256>;
    async fn new_block_filter(&self) -> Result<U256>;
    // misc
    async fn get_client_version(&self) -> String;
    async fn get_chain_id(&self) -> u64;
    async fn get_coinbase(&self) -> Result<Address>;
    async fn syncing(&self) -> Result<SyncStatus>;
}
