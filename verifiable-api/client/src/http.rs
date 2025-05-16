use std::marker::PhantomData;

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
pub struct HttpVerifiableApi<N: NetworkSpec> {
    client: Client,
    base_url: String,
    phantom: PhantomData<N>,
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec> VerifiableApi<N> for HttpVerifiableApi<N> {
    fn new(base_url: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches("/").to_string(),
            phantom: PhantomData,
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
            request = request.query(&[("block", block_id)]);
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
        let url = format!("{}/eth/v1/proof/receipt/{}", self.base_url, tx_hash);
        let response = self.client.get(&url).send().await?;
        handle_response(response).await
    }

    async fn get_transaction(&self, tx_hash: B256) -> Result<Option<TransactionResponse<N>>> {
        let url = format!("{}/eth/v1/proof/transaction/{}", self.base_url, tx_hash);
        let response = self.client.get(&url).send().await?;
        handle_response(response).await
    }

    async fn get_transaction_by_location(
        &self,
        block_id: BlockId,
        index: u64,
    ) -> Result<Option<TransactionResponse<N>>> {
        let url = format!(
            "{}/eth/v1/proof/transaction/{}/{}",
            self.base_url,
            serialize_block_id(block_id),
            index
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

    async fn get_execution_hint(
        &self,
        tx: N::TransactionRequest,
        validate_tx: bool,
        block_id: Option<BlockId>,
    ) -> Result<ExtendedAccessListResponse> {
        let url = format!("{}/eth/v1/proof/getExecutionHint", self.base_url);
        let response = self
            .client
            .post(&url)
            .json(&ExtendedAccessListRequest::<N> {
                tx,
                validate_tx,
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

    async fn get_block(
        &self,
        block_id: BlockId,
        full_tx: bool,
    ) -> Result<Option<N::BlockResponse>> {
        let url = format!(
            "{}/eth/v1/block/{}",
            self.base_url,
            serialize_block_id(block_id)
        );
        let response = self
            .client
            .get(&url)
            .query(&[("transactionDetailFlag", full_tx)])
            .send()
            .await?;
        handle_response(response).await
    }

    async fn get_block_receipts(
        &self,
        block_id: BlockId,
    ) -> Result<Option<Vec<N::ReceiptResponse>>> {
        let url = format!(
            "{}/eth/v1/block/{}/receipts",
            self.base_url,
            serialize_block_id(block_id)
        );
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
}

async fn handle_response<T: DeserializeOwned>(response: Response) -> Result<T> {
    if response.status().is_success() {
        Ok(response.json::<T>().await?)
    } else {
        let error_response = response.json::<ErrorResponse>().await?;
        Err(eyre!(error_response.error.to_string()))
    }
}

fn serialize_block_id(block_id: BlockId) -> String {
    match block_id {
        BlockId::Number(number) => serde_json::to_string(&number).unwrap(),
        BlockId::Hash(hash) => hash.block_hash.to_string(),
    }
}
