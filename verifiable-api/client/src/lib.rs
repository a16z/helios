use alloy::{
    eips::BlockId,
    primitives::{Address, B256, U256},
    rpc::types::{Filter, ValueOrArray},
};
use async_trait::async_trait;
use eyre::Result;
use reqwest::Client;

use helios_common::network_spec::NetworkSpec;
use helios_verifiable_api_types::*;

// re-export types
pub use helios_verifiable_api_types as types;

#[async_trait]
pub trait VerifiableApi<N: NetworkSpec>: Send + Clone + Sync {
    fn new(base_url: &str) -> Self
    where
        Self: Sized;
    async fn get_account(
        &self,
        address: Address,
        storage_slots: &[U256],
        block: Option<BlockId>,
    ) -> Result<AccountResponse>;
    async fn get_transaction_receipt(
        &self,
        tx_hash: B256,
    ) -> Result<Option<TransactionReceiptResponse<N>>>;
    async fn get_logs(&self, filter: &Filter) -> Result<LogsResponse<N>>;
    async fn get_filter_logs(&self, filter_id: U256) -> Result<FilterLogsResponse<N>>;
    async fn get_filter_changes(&self, filter_id: U256) -> Result<FilterChangesResponse<N>>;
    async fn create_access_list(
        &self,
        tx: N::TransactionRequest,
        block: Option<BlockId>,
    ) -> Result<AccessListResponse>;
}

pub struct VerifiableApiClient {
    client: Client,
    base_url: String,
}

impl Clone for VerifiableApiClient {
    fn clone(&self) -> Self {
        Self {
            client: Client::new(),
            base_url: self.base_url.to_string(),
        }
    }
}

#[async_trait]
impl<N: NetworkSpec> VerifiableApi<N> for VerifiableApiClient {
    fn new(base_url: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.to_string(),
        }
    }

    async fn get_account(
        &self,
        address: Address,
        storage_slots: &[U256],
        block: Option<BlockId>,
    ) -> Result<AccountResponse> {
        let url = format!("{}/eth/v1/proof/account/{}", self.base_url, address);
        let mut request = self.client.get(&url);
        if let Some(block) = block {
            request = request.query(&[("block", block.to_string())]);
        }
        for slot in storage_slots {
            request = request.query(&[("storageSlots", slot)]);
        }
        let response = request.send().await?;
        let response = response.json::<AccountResponse>().await?;
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
            request = request.query(&[("fromBlock", U256::from(from_block))]);
        }
        if let Some(to_block) = filter.get_to_block() {
            request = request.query(&[("toBlock", U256::from(to_block))]);
        }
        if let Some(block_hash) = filter.get_block_hash() {
            request = request.query(&[("blockHash", block_hash)]);
        }
        if let Some(address) = filter.address.to_value_or_array() {
            request = request.query(&[("address", address)]);
        }
        for idx in 0..=3 {
            if let Some(topics) = filter.topics[idx].to_value_or_array() {
                match topics {
                    ValueOrArray::Value(topic) => {
                        request = request.query(&[(format!("topic{}", idx), topic)]);
                    }
                    ValueOrArray::Array(topics) => {
                        for topic in topics {
                            request = request.query(&[(format!("topic{}", idx), topic)]);
                        }
                    }
                }
            }
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

    async fn create_access_list(
        &self,
        tx: N::TransactionRequest,
        block: Option<BlockId>,
    ) -> Result<AccessListResponse> {
        let url = format!("{}/eth/v1/proof/create_access_list", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&AccessListRequest::<N> { tx, block })
            .send()
            .await?;
        let response = response.json::<AccessListResponse>().await?;
        Ok(response)
    }
}
