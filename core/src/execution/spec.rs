use std::collections::HashMap;

use alloy::{
    eips::BlockId,
    primitives::{Address, B256, U256},
    rpc::types::{Filter, FilterChanges, Log},
};
use async_trait::async_trait;
use eyre::Result;
use helios_common::{
    network_spec::NetworkSpec,
    types::{Account, BlockTag},
};

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait ExecutionSpec<N: NetworkSpec>: Send + Sync + 'static {
    async fn get_account(
        &self,
        address: Address,
        slots: Option<&[B256]>,
        tag: BlockTag,
        include_code: bool,
    ) -> Result<Account>;
    async fn get_transaction_receipt(&self, tx_hash: B256) -> Result<Option<N::ReceiptResponse>>;
    async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>>;
    async fn get_filter_changes(&self, filter_id: U256) -> Result<FilterChanges>;
    async fn get_filter_logs(&self, filter_id: U256) -> Result<Vec<Log>>;
    async fn create_extended_access_list(
        &self,
        tx: &N::TransactionRequest,
        validate_tx: bool,
        block_id: Option<BlockId>,
    ) -> Result<HashMap<Address, Account>>;
    async fn chain_id(&self) -> Result<u64>;
    async fn get_block(&self, block_id: BlockId, full_tx: bool)
        -> Result<Option<N::BlockResponse>>;
    async fn get_untrusted_block(
        &self,
        block_id: BlockId,
        full_tx: bool,
    ) -> Result<Option<N::BlockResponse>>;
    async fn get_block_receipts(
        &self,
        block_id: BlockId,
    ) -> Result<Option<Vec<N::ReceiptResponse>>>;
    async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<B256>;
    async fn new_filter(&self, filter: &Filter) -> Result<U256>;
    async fn new_block_filter(&self) -> Result<U256>;
    async fn new_pending_transaction_filter(&self) -> Result<U256>;
    async fn uninstall_filter(&self, filter_id: U256) -> Result<bool>;
}
