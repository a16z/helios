use std::{marker::PhantomData, sync::{Arc, Weak}};

use alloy::{
    eips::BlockId,
    primitives::{Address, B256, U256},
    rpc::types::{Filter, ValueOrArray},
};
use async_trait::async_trait;
use eyre::{eyre, Result};
use reqwest::{self, Response};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use serde::de::DeserializeOwned;
#[cfg(not(target_arch = "wasm32"))]
use tokio::time::{Duration, interval, timeout};
use url::Url;

use helios_common::network_spec::NetworkSpec;
use helios_verifiable_api_types::*;

use super::VerifiableApi;

#[derive(Clone)]
pub struct HttpVerifiableApi<N: NetworkSpec> {
    client: Arc<ClientWithMiddleware>,
    base_url: Url,
    phantom: PhantomData<N>,
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec> VerifiableApi<N> for HttpVerifiableApi<N> {
    fn new(base_url: &Url) -> Self {
        let builder = reqwest::ClientBuilder::default();

        #[cfg(not(target_arch = "wasm32"))]
        let builder = builder
            // Keep 30 connections ready per host
            .pool_max_idle_per_host(30)
            // Disable idle timeout - keep connections indefinitely
            .pool_idle_timeout(None)
            // Aggressive TCP keepalive
            .tcp_keepalive(Some(Duration::from_secs(30)))
            // HTTP/2 specific settings
            .http2_keep_alive_interval(Some(Duration::from_secs(30)))
            .http2_keep_alive_timeout(Duration::from_secs(30))
            .http2_keep_alive_while_idle(true)
            // Prevent connection closure
            .tcp_nodelay(true)
            // Fast decompression
            .brotli(true)
            // Faster DNS resolution
            .hickory_dns(true)
            // Use http2
            .http2_prior_knowledge();

        let client = builder.build().expect("Failed to build HTTP client");

        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
        let client = Arc::new(
            ClientBuilder::new(client)
                .with(RetryTransientMiddleware::new_with_policy(retry_policy))
                .build(),
        );

        #[cfg(not(target_arch = "wasm32"))]
        {
            let client_weak: Weak<ClientWithMiddleware> = Arc::downgrade(&client);
            let base_url_str = base_url.to_string();
            tokio::spawn(async move {
                let mut ticker = interval(Duration::from_secs(30));
                let mut backoff_secs: u64 = 0; // 0 means no extra backoff
                const TIMEOUT_SECS: u64 = 2;
                const BACKOFF_START: u64 = 5;
                const BACKOFF_CAP: u64 = 300; // 5 minutes
                
                loop {
                    // If the client has been dropped everywhere, exit the task
                    let Some(client_strong) = client_weak.upgrade() else { break; };

                    // Base cadence
                    ticker.tick().await;

                    // Apply additional backoff if in error state
                    if backoff_secs > 0 {
                        tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
                    }

                    // Make a lightweight HEAD request with timeout
                    let attempt = timeout(
                        Duration::from_secs(TIMEOUT_SECS),
                        client_strong.head(&base_url_str).send(),
                    ).await;

                    match attempt {
                        Ok(Ok(_)) => {
                            // Success clears backoff
                            backoff_secs = 0;
                        }
                        _ => {
                            // Increase backoff exponentially with cap
                            backoff_secs = if backoff_secs == 0 { BACKOFF_START } else { (backoff_secs.saturating_mul(2)).min(BACKOFF_CAP) };
                        }
                    }
                }
            });
        }

        Self {
            client,
            base_url: base_url.clone(),
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
        let url = self
            .base_url
            .join(&format!("eth/v1/proof/account/{address}"))
            .map_err(|e| eyre!("Failed to construct account URL: {}", e))?;
        let mut request = self.client.get(url.as_str());
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
        let url = self
            .base_url
            .join(&format!("eth/v1/proof/receipt/{tx_hash}"))
            .map_err(|e| eyre!("Failed to construct receipt URL: {}", e))?;
        let response = self.client.get(url.as_str()).send().await?;
        handle_response(response).await
    }

    async fn get_transaction(&self, tx_hash: B256) -> Result<Option<TransactionResponse<N>>> {
        let url = self
            .base_url
            .join(&format!("eth/v1/proof/transaction/{tx_hash}"))
            .map_err(|e| eyre!("Failed to construct transaction URL: {}", e))?;
        let response = self.client.get(url.as_str()).send().await?;
        handle_response(response).await
    }

    async fn get_transaction_by_location(
        &self,
        block_id: BlockId,
        index: u64,
    ) -> Result<Option<TransactionResponse<N>>> {
        let url = self
            .base_url
            .join(&format!(
                "eth/v1/proof/transaction/{}/{}",
                serialize_block_id(block_id),
                index
            ))
            .map_err(|e| eyre!("Failed to construct transaction by location URL: {}", e))?;
        let response = self.client.get(url.as_str()).send().await?;
        handle_response(response).await
    }

    async fn get_logs(&self, filter: &Filter) -> Result<LogsResponse<N>> {
        let url = self
            .base_url
            .join("eth/v1/proof/logs")
            .map_err(|e| eyre!("Failed to construct logs URL: {}", e))?;

        let mut request = self.client.get(url.as_str());
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
                        request = request.query(&[(format!("topic{idx}"), topic)]);
                    }
                    ValueOrArray::Array(topics) => {
                        for topic in topics {
                            request = request.query(&[(format!("topic{idx}"), topic)]);
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
        let url = self
            .base_url
            .join("eth/v1/proof/getExecutionHint")
            .map_err(|e| eyre!("Failed to construct execution hint URL: {}", e))?;
        let response = self
            .client
            .post(url.as_str())
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
        let url = self
            .base_url
            .join("eth/v1/chainId")
            .map_err(|e| eyre!("Failed to construct chain ID URL: {}", e))?;
        let response = self.client.get(url.as_str()).send().await?;
        handle_response(response).await
    }

    async fn get_block(
        &self,
        block_id: BlockId,
        full_tx: bool,
    ) -> Result<Option<N::BlockResponse>> {
        let url = self
            .base_url
            .join(&format!("eth/v1/block/{}", serialize_block_id(block_id)))
            .map_err(|e| eyre!("Failed to construct block URL: {}", e))?;

        let request = self
            .client
            .get(url.as_str())
            .query(&[("transactionDetailFlag", full_tx)]);

        let response = request.send().await?;

        handle_response(response).await
    }

    async fn get_block_receipts(
        &self,
        block_id: BlockId,
    ) -> Result<Option<Vec<N::ReceiptResponse>>> {
        let url = self
            .base_url
            .join(&format!(
                "eth/v1/block/{}/receipts",
                serialize_block_id(block_id)
            ))
            .map_err(|e| eyre!("Failed to construct block receipts URL: {}", e))?;
        let response = self.client.get(url.as_str()).send().await?;
        handle_response(response).await
    }

    async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<SendRawTxResponse> {
        let url = self
            .base_url
            .join("eth/v1/sendRawTransaction")
            .map_err(|e| eyre!("Failed to construct send transaction URL: {}", e))?;
        let response = self
            .client
            .post(url.as_str())
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
        let bytes = response.bytes().await?;
        Ok(serde_json::from_slice(&bytes)?)
    } else {
        let error_response = response.json::<ErrorResponse>().await?;
        Err(eyre!(error_response.error.to_string()))
    }
}

fn serialize_block_id(block_id: BlockId) -> String {
    match block_id {
        BlockId::Hash(hash) => hash.block_hash.to_string(),
        BlockId::Number(number) => serde_json::to_string(&number)
            .expect("Failed to serialize block number")
            .trim_matches('"')
            .to_string(),
    }
}
