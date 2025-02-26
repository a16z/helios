use alloy::{
    eips::BlockId,
    primitives::{Address, B256, U256},
    rpc::types::{Filter, ValueOrArray},
};
use async_trait::async_trait;
use eyre::{eyre, Result};
use reqwest::{Client, Response};
use serde::de::DeserializeOwned;

use helios_common::network_spec::NetworkSpec;
use helios_verifiable_api_types::*;

use super::VerifiableApi;

#[derive(Clone)]
pub struct HttpVerifiableApi {
    client: Client,
    base_url: String,
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec> VerifiableApi<N> for HttpVerifiableApi {
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
        block_id: Option<BlockId>,
        include_code: bool,
    ) -> Result<AccountResponse> {
        let url = format!("{}/eth/v1/proof/account/{}", self.base_url, address);
        let mut request = self.client.get(&url);
        if let Some(block_id) = block_id {
            request = request.query(&[("block", block_id.to_string())]);
        }
        for slot in storage_slots {
            request = request.query(&[("storageSlots", slot)]);
        }
        request = request.query(&[("includeCode", include_code)]);
        let response = request.send().await?;
        handle_response(response).await
    }

    async fn get_transaction_receipt(
        &self,
        tx_hash: B256,
    ) -> Result<Option<TransactionReceiptResponse<N>>> {
        let url = format!(
            "{}/eth/v1/proof/transaction/{}/receipt",
            self.base_url, tx_hash
        );
        let response = self.client.get(&url).send().await?;
        handle_response(response).await
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
        handle_response(response).await
    }

    async fn get_filter_logs(&self, filter_id: U256) -> Result<FilterLogsResponse<N>> {
        let url = format!("{}/eth/v1/proof/filterLogs/{}", self.base_url, filter_id);
        let response = self.client.get(&url).send().await?;
        handle_response(response).await
    }

    async fn get_filter_changes(&self, filter_id: U256) -> Result<FilterChangesResponse<N>> {
        let url = format!("{}/eth/v1/proof/filterChanges/{}", self.base_url, filter_id);
        let response = self.client.get(&url).send().await?;
        handle_response(response).await
    }

    async fn create_access_list(
        &self,
        tx: N::TransactionRequest,
        block_id: Option<BlockId>,
    ) -> Result<AccessListResponse> {
        let url = format!("{}/eth/v1/proof/createAccessList", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&AccessListRequest::<N> {
                tx,
                block: block_id,
            })
            .send()
            .await?;
        handle_response(response).await
    }

    async fn chain_id(&self) -> Result<ChainIdResponse> {
        let url = format!("{}/eth/v1/chainId", self.base_url);
        let response = self.client.get(&url).send().await?;
        handle_response(response).await
    }

    async fn get_block(&self, block_id: BlockId) -> Result<Option<N::BlockResponse>> {
        let url = format!("{}/eth/v1/block/{}", self.base_url, block_id);
        let response = self.client.get(&url).send().await?;
        handle_response(response).await
    }

    async fn get_block_receipts(
        &self,
        block_id: BlockId,
    ) -> Result<Option<Vec<N::ReceiptResponse>>> {
        let url = format!("{}/eth/v1/block/{}/receipts", self.base_url, block_id);
        let response = self.client.get(&url).send().await?;
        handle_response(response).await
    }

    async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<SendRawTxResponse> {
        let url = format!("{}/eth/v1/sendRawTransaction", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&SendRawTxRequest {
                bytes: bytes.to_vec(),
            })
            .send()
            .await?;
        handle_response(response).await
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
        handle_response(response).await
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
        handle_response(response).await
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
        handle_response(response).await
    }

    async fn uninstall_filter(&self, filter_id: U256) -> Result<UninstallFilterResponse> {
        let url = format!("{}/eth/v1/filter/{}", self.base_url, filter_id);
        let response = self.client.delete(&url).send().await?;
        handle_response(response).await
    }
}

async fn handle_response<T: DeserializeOwned>(response: Response) -> Result<T> {
    if response.status().is_success() {
        Ok(response.json::<T>().await?)
    } else {
        let error_response = response.json::<ErrorResponse>().await?;
        Err(eyre!(error_response.error.to_string()))
    }
}
