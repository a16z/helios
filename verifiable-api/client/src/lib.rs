use alloy::{
    eips::BlockId,
    primitives::{Address, B256, U256},
    rpc::types::Filter,
};
use async_trait::async_trait;
use eyre::Result;
use reqwest::Client;

use helios_common::network_spec::NetworkSpec;
use helios_verifiable_api_types::*;

// re-export types
pub use helios_verifiable_api_types as types;

#[async_trait]
pub trait VerifiableApi<N: NetworkSpec>: Send + Clone + Sync + 'static {
    async fn get_account(
        &self,
        address: Address,
        storage_keys: &[B256],
        block: Option<BlockId>,
    ) -> Result<AccountResponse>;
    async fn get_storage_at(
        &self,
        address: Address,
        key: U256,
        block: Option<BlockId>,
    ) -> Result<StorageAtResponse>;
    async fn get_block_receipts(&self, block: BlockId) -> Result<BlockReceiptsResponse<N>>;
    async fn get_transaction_receipt(
        &self,
        tx_hash: B256,
    ) -> Result<Option<TransactionReceiptResponse<N>>>;
    async fn get_logs(&self, filter: &Filter) -> Result<LogsResponse<N>>;
    async fn get_filter_logs(&self, filter_id: U256) -> Result<FilterLogsResponse<N>>;
    async fn get_filter_changes(&self, filter_id: U256) -> Result<FilterChangesResponse<N>>;
}

#[derive(Clone)]
pub struct VerifiableApiClient {
    client: Client,
    base_url: String,
}

impl VerifiableApiClient {
    pub fn new(base_url: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
        }
    }
}

#[async_trait]
impl<N: NetworkSpec> VerifiableApi<N> for VerifiableApiClient {
    async fn get_account(
        &self,
        address: Address,
        storage_keys: &[B256],
        block: Option<BlockId>,
    ) -> Result<AccountResponse> {
        let url = format!("{}/eth/v1/proof/account/{}", self.base_url, address);
        let response = self
            .client
            .get(&url)
            .query(&[("block", block)])
            .query(&[("storageKeys", storage_keys)])
            .send()
            .await?;
        let response = response.json::<AccountResponse>().await?;
        Ok(response)
    }

    async fn get_storage_at(
        &self,
        address: Address,
        key: U256,
        block: Option<BlockId>,
    ) -> Result<StorageAtResponse> {
        let url = format!("{}/eth/v1/proof/storage/{}/{}", self.base_url, address, key);
        let response = self
            .client
            .get(&url)
            .query(&[("block", block)])
            .send()
            .await?;
        let response = response.json::<StorageAtResponse>().await?;
        Ok(response)
    }

    async fn get_block_receipts(&self, block: BlockId) -> Result<BlockReceiptsResponse<N>> {
        let url = format!("{}/eth/v1/proof/block_receipts/{}", self.base_url, block);
        let response = self.client.get(&url).send().await?;
        let response = response.json::<BlockReceiptsResponse<N>>().await?;
        Ok(response)
    }

    async fn get_transaction_receipt(
        &self,
        tx_hash: B256,
    ) -> Result<Option<TransactionReceiptResponse<N>>> {
        let url = format!("{}/eth/v1/proof/tx_receipt/{}", self.base_url, tx_hash);
        let response = self.client.get(&url).send().await?;
        let response = response
            .json::<Option<TransactionReceiptResponse<N>>>()
            .await?;
        Ok(response)
    }

    async fn get_logs(&self, filter: &Filter) -> Result<LogsResponse<N>> {
        let url = format!("{}/eth/v1/proof/logs", self.base_url);

        let mut request = self.client.get(&url);
        if let Some(from_block) = filter.get_from_block() {
            request = request.query(&[("fromBlock", from_block)]);
        }
        if let Some(to_block) = filter.get_to_block() {
            request = request.query(&[("toBlock", to_block)]);
        }
        if let Some(block_hash) = filter.get_block_hash() {
            request = request.query(&[("blockHash", block_hash)]);
        }
        if let Some(address) = filter.address.to_value_or_array() {
            request = request.query(&[("address", address)]);
        }
        let topics = filter
            .topics
            .iter()
            .filter_map(|t| t.to_value_or_array())
            .collect::<Vec<_>>();
        if topics.len() > 0 {
            request = request.query(&[("topics", topics)]);
        }

        let response = request.send().await?;
        let response = response.json::<LogsResponse<N>>().await?;
        Ok(response)
    }

    async fn get_filter_logs(&self, filter_id: U256) -> Result<FilterLogsResponse<N>> {
        let url = format!("{}/eth/v1/proof/filter_logs/{}", self.base_url, filter_id);
        let response = self.client.get(&url).send().await?;
        let response = response.json::<FilterLogsResponse<N>>().await?;
        Ok(response)
    }

    async fn get_filter_changes(&self, filter_id: U256) -> Result<FilterChangesResponse<N>> {
        let url = format!(
            "{}/eth/v1/proof/filter_changes/{}",
            self.base_url, filter_id
        );
        let response = self.client.get(&url).send().await?;
        let response = response.json::<FilterChangesResponse<N>>().await?;
        Ok(response)
    }
}
