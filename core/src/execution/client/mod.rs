use alloy::primitives::{Address, B256, U256};
use alloy::rpc::types::{Filter, FilterChanges, Log};
use async_trait::async_trait;
use eyre::Result;

use helios_common::{
    network_spec::NetworkSpec,
    types::{Account, BlockTag},
};

#[async_trait]
pub trait VerifiableMethods<N: NetworkSpec>: Send + Clone + Sync + 'static {
    async fn get_account(
        &self,
        address: Address,
        slots: Option<&[B256]>,
        tag: BlockTag,
    ) -> Result<Account>;
    async fn get_transaction_receipt(&self, tx_hash: B256) -> Result<Option<N::ReceiptResponse>>;
    async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>>;
    async fn get_filter_changes(&self, filter_id: U256) -> Result<FilterChanges>;
    async fn get_filter_logs(&self, filter_id: U256) -> Result<Vec<Log>>;
}
