use alloy::{
    eips::BlockId,
    primitives::{Address, B256, U256},
    rpc::types::Filter,
};
use async_trait::async_trait;
use eyre::Result;
use reqwest::Client;

use helios_core::network_spec::NetworkSpec;

use crate::types::*;

#[async_trait]
pub trait VerifiableApi<N: NetworkSpec> {
    async fn get_account(
        &self,
        address: Address,
        storage_keys: Vec<B256>,
        block: Option<BlockId>,
    ) -> Result<GetAccountProofResponse>;
    async fn get_balance(
        &self,
        address: Address,
        block: Option<BlockId>,
    ) -> Result<GetBalanceResponse>;
    async fn get_transaction_count(
        &self,
        address: Address,
        block: Option<BlockId>,
    ) -> Result<GetTransactionCountResponse>;
    async fn get_code(&self, address: Address, block: Option<BlockId>) -> Result<GetCodeResponse>;
    async fn get_storage_at(
        &self,
        address: Address,
        key: U256,
        block: Option<BlockId>,
    ) -> Result<GetStorageAtResponse>;
    async fn get_block_receipts(&self, block: BlockId) -> Result<GetBlockReceiptsResponse<N>>;
    async fn get_transaction_receipt(
        &self,
        tx_hash: B256,
    ) -> Result<GetTransactionReceiptResponse<N>>;
    async fn get_logs(&self, filter: Filter) -> Result<GetLogsResponse<N>>;
    async fn get_filter_logs(&self, filter_id: U256) -> Result<GetFilterLogsResponse<N>>;
    async fn get_filter_changes(&self, filter_id: U256) -> Result<GetFilterChangesResponse<N>>;
}

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
        storage_keys: Vec<B256>,
        block: Option<BlockId>,
    ) -> Result<GetAccountProofResponse> {
        let url = format!("{}/eth/v1/proof/account/{}", self.base_url, address);
        let response = self
            .client
            .get(&url)
            .query(&[("block", block)])
            .query(&[("storageKeys", &storage_keys)])
            .send()
            .await?;
        let response = response.json::<GetAccountProofResponse>().await?;
        Ok(response)
    }

    async fn get_balance(
        &self,
        address: Address,
        block: Option<BlockId>,
    ) -> Result<GetBalanceResponse> {
        let url = format!("{}/eth/v1/proof/balance/{}", self.base_url, address);
        let response = self
            .client
            .get(&url)
            .query(&[("block", block)])
            .send()
            .await?;
        let response = response.json::<GetBalanceResponse>().await?;
        Ok(response)
    }

    async fn get_transaction_count(
        &self,
        address: Address,
        block: Option<BlockId>,
    ) -> Result<GetTransactionCountResponse> {
        let url = format!(
            "{}/eth/v1/proof/transaction_count/{}",
            self.base_url, address
        );
        let response = self
            .client
            .get(&url)
            .query(&[("block", block)])
            .send()
            .await?;
        let response = response.json::<GetTransactionCountResponse>().await?;
        Ok(response)
    }

    async fn get_code(&self, address: Address, block: Option<BlockId>) -> Result<GetCodeResponse> {
        let url = format!("{}/eth/v1/proof/code/{}", self.base_url, address);
        let response = self
            .client
            .get(&url)
            .query(&[("block", block)])
            .send()
            .await?;
        let response = response.json::<GetCodeResponse>().await?;
        Ok(response)
    }

    async fn get_storage_at(
        &self,
        address: Address,
        key: U256,
        block: Option<BlockId>,
    ) -> Result<GetStorageAtResponse> {
        let url = format!("{}/eth/v1/proof/storage/{}/{}", self.base_url, address, key);
        let response = self
            .client
            .get(&url)
            .query(&[("block", block)])
            .send()
            .await?;
        let response = response.json::<GetStorageAtResponse>().await?;
        Ok(response)
    }

    async fn get_block_receipts(&self, block: BlockId) -> Result<GetBlockReceiptsResponse<N>> {
        let url = format!("{}/eth/v1/proof/block_receipts/{}", self.base_url, block);
        let response = self.client.get(&url).send().await?;
        let response = response.json::<GetBlockReceiptsResponse<N>>().await?;
        Ok(response)
    }

    async fn get_transaction_receipt(
        &self,
        tx_hash: B256,
    ) -> Result<GetTransactionReceiptResponse<N>> {
        let url = format!("{}/eth/v1/proof/tx_receipt/{}", self.base_url, tx_hash);
        let response = self.client.get(&url).send().await?;
        let response = response.json::<GetTransactionReceiptResponse<N>>().await?;
        Ok(response)
    }

    async fn get_logs(&self, filter: Filter) -> Result<GetLogsResponse<N>> {
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
            .into_iter()
            .filter_map(|t| t.to_value_or_array())
            .collect::<Vec<_>>();
        if topics.len() > 0 {
            request = request.query(&[("topics", topics)]);
        }

        let response = request.send().await?;
        let response = response.json::<GetLogsResponse<N>>().await?;
        Ok(response)
    }

    async fn get_filter_logs(&self, filter_id: U256) -> Result<GetFilterLogsResponse<N>> {
        let url = format!("{}/eth/v1/proof/filter_logs/{}", self.base_url, filter_id);
        let response = self.client.get(&url).send().await?;
        let response = response.json::<GetFilterLogsResponse<N>>().await?;
        Ok(response)
    }

    async fn get_filter_changes(&self, filter_id: U256) -> Result<GetFilterChangesResponse<N>> {
        let url = format!(
            "{}/eth/v1/proof/filter_changes/{}",
            self.base_url, filter_id
        );
        let response = self.client.get(&url).send().await?;
        let response = response.json::<GetFilterChangesResponse<N>>().await?;
        Ok(response)
    }
}
