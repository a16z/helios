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
        block: Option<BlockId>,
        include_code: bool,
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
    // Methods just for compatibility (acts as a proxy)
    async fn chain_id(&self) -> Result<ChainIdResponse>;
    async fn get_block(&self, block_id: BlockId) -> Result<Option<N::BlockResponse>>;
    async fn get_block_receipts(&self, block: BlockId) -> Result<Option<Vec<N::ReceiptResponse>>>;
    async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<SendRawTxResponse>;
    async fn new_filter(&self, filter: &Filter) -> Result<NewFilterResponse>;
    async fn new_block_filter(&self) -> Result<NewFilterResponse>;
    async fn new_pending_transaction_filter(&self) -> Result<NewFilterResponse>;
    async fn uninstall_filter(&self, filter_id: U256) -> Result<UninstallFilterResponse>;
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

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec> VerifiableApi<N> for VerifiableApiClient {
    fn new(base_url: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches("/").to_string(),
        }
    }

    async fn get_account(
        &self,
        address: Address,
        storage_slots: &[U256],
        block: Option<BlockId>,
        include_code: bool,
    ) -> Result<AccountResponse> {
        let url = format!("{}/eth/v1/proof/account/{}", self.base_url, address);
        let mut request = self.client.get(&url);
        if let Some(block) = block {
            request = request.query(&[("block", block.to_string())]);
        }
        for slot in storage_slots {
            request = request.query(&[("storageSlots", slot)]);
        }
        request = request.query(&[("includeCode", include_code)]);
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
            match address {
                ValueOrArray::Value(address) => {
                    request = request.query(&[("address", address)]);
                }
                ValueOrArray::Array(addresses) => {
                    for address in addresses {
                        request = request.query(&[("address", address)]);
                    }
                }
            }
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

    async fn chain_id(&self) -> Result<ChainIdResponse> {
        let url = format!("{}/eth/v1/chain_id", self.base_url);
        let response = self.client.get(&url).send().await?;
        let response = response.json::<ChainIdResponse>().await?;
        Ok(response)
    }

    async fn get_block(&self, block_id: BlockId) -> Result<Option<N::BlockResponse>> {
        let url = format!("{}/eth/v1/block/{}", self.base_url, block_id);
        let response = self.client.get(&url).send().await?;
        let response = response.json::<Option<N::BlockResponse>>().await?;
        Ok(response)
    }

    async fn get_block_receipts(&self, block: BlockId) -> Result<Option<Vec<N::ReceiptResponse>>> {
        let url = format!("{}/eth/v1/block/{}/receipts", self.base_url, block);
        let response = self.client.get(&url).send().await?;
        let response = response.json::<Option<Vec<N::ReceiptResponse>>>().await?;
        Ok(response)
    }

    async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<SendRawTxResponse> {
        let url = format!("{}/eth/v1/send_raw_transaction", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&SendRawTxRequest {
                bytes: bytes.to_vec(),
            })
            .send()
            .await?;
        let response = response.json::<SendRawTxResponse>().await?;
        Ok(response)
    }

    async fn new_filter(&self, filter: &Filter) -> Result<NewFilterResponse> {
        let url = format!("{}/eth/v1/filter", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&NewFilterRequest {
                kind: FilterKind::Logs,
                filter: Some(filter.clone()),
            })
            .send()
            .await?;
        let response = response.json::<NewFilterResponse>().await?;
        Ok(response)
    }

    async fn new_block_filter(&self) -> Result<NewFilterResponse> {
        let url = format!("{}/eth/v1/filter", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&NewFilterRequest {
                kind: FilterKind::NewBlocks,
                filter: None,
            })
            .send()
            .await?;
        let response = response.json::<NewFilterResponse>().await?;
        Ok(response)
    }

    async fn new_pending_transaction_filter(&self) -> Result<NewFilterResponse> {
        let url = format!("{}/eth/v1/filter", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&NewFilterRequest {
                kind: FilterKind::NewPendingTransactions,
                filter: None,
            })
            .send()
            .await?;
        let response = response.json::<NewFilterResponse>().await?;
        Ok(response)
    }

    async fn uninstall_filter(&self, filter_id: U256) -> Result<UninstallFilterResponse> {
        let url = format!("{}/eth/v1/filter/{}", self.base_url, filter_id);
        let response = self.client.delete(&url).send().await?;
        let response = response.json::<UninstallFilterResponse>().await?;
        Ok(response)
    }
}
