use alloy::{
    primitives::{Address, Bytes, B256, U256},
    rpc::types::{AccessListResult, EIP1186AccountProofResponse, Filter, Log, SyncStatus},
};
use async_trait::async_trait;
use eyre::Result;

use helios_common::{
    network_spec::NetworkSpec,
    types::{BlockTag, SubEventRx, SubscriptionType},
};

#[async_trait]
pub trait HeliosApi<N: NetworkSpec>: Send + Sync + 'static {
    // node management
    async fn wait_synced(&self);
    async fn shutdown(&self);
    // standard rpc methods
    async fn subscribe(&self, sub_type: SubscriptionType) -> Result<SubEventRx<N>>;
    async fn call(&self, tx: &N::TransactionRequest, block: BlockTag) -> Result<Bytes>;
    async fn estimate_gas(&self, tx: &N::TransactionRequest, block: BlockTag) -> Result<u64>;
    async fn create_access_list(
        &self,
        tx: &N::TransactionRequest,
        block: BlockTag,
    ) -> Result<AccessListResult>;
    async fn get_balance(&self, address: Address, block: BlockTag) -> Result<U256>;
    async fn get_nonce(&self, address: Address, block: BlockTag) -> Result<u64>;
    async fn get_block_transaction_count_by_hash(&self, hash: B256) -> Result<Option<u64>>;
    async fn get_block_transaction_count_by_number(&self, block: BlockTag) -> Result<Option<u64>>;
    async fn get_code(&self, address: Address, block: BlockTag) -> Result<Bytes>;
    async fn get_storage_at(&self, address: Address, slot: U256, block: BlockTag) -> Result<B256>;
    async fn get_proof(
        &self,
        address: Address,
        slots: &[B256],
        block: BlockTag,
    ) -> Result<EIP1186AccountProofResponse>;
    async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<B256>;
    async fn get_transaction_receipt(&self, tx_hash: B256) -> Result<Option<N::ReceiptResponse>>;
    async fn get_block_receipts(&self, block: BlockTag) -> Result<Vec<N::ReceiptResponse>>;
    async fn get_transaction_by_hash(
        &self,
        tx_hash: B256,
    ) -> Result<Option<N::TransactionResponse>>;
    async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>>;
    async fn get_filter_logs(&self, filter_id: U256) -> Result<Vec<Log>>;
    async fn uninstall_filter(&self, filter_id: U256) -> Result<bool>;
    async fn new_filter(&self, filter: &Filter) -> Result<U256>;
    async fn new_block_filter(&self) -> Result<U256>;
    async fn new_pending_transaction_filter(&self) -> Result<U256>;
    async fn get_gas_price(&self) -> Result<U256>;
    async fn get_priority_fee(&self) -> Result<U256>;
    async fn blob_base_fee(&self, block: BlockTag) -> Result<U256>;
    async fn get_block_number(&self) -> Result<U256>;
    async fn client_version(&self) -> String;
    async fn get_block_by_number(
        &self,
        block: BlockTag,
        full_tx: bool,
    ) -> Result<Option<N::BlockResponse>>;
    async fn get_block_by_hash(
        &self,
        hash: B256,
        full_tx: bool,
    ) -> Result<Option<N::BlockResponse>>;
    async fn get_transaction_by_block_hash_and_index(
        &self,
        block_hash: B256,
        index: u64,
    ) -> Result<Option<N::TransactionResponse>>;
    async fn get_transaction_by_block_number_and_index(
        &self,
        block: BlockTag,
        index: u64,
    ) -> Result<Option<N::TransactionResponse>>;
    async fn chain_id(&self) -> u64;
    async fn syncing(&self) -> Result<SyncStatus>;
    async fn get_coinbase(&self) -> Result<Address>;
}
