use alloy::eips::BlockNumberOrTag;
use alloy::primitives::{Address, B256, U256};
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::rpc::client::ClientBuilder;
use alloy::rpc::types::{BlockId, EIP1186AccountProofResponse, FeeHistory, Filter, Log};
use alloy::transports::http::Http;
use alloy::transports::layers::{RetryBackoffLayer, RetryBackoffService};
use async_trait::async_trait;
use eyre::{eyre, Result};
use reqwest::Client;
use revm::primitives::AccessList;

use crate::errors::RpcError;
use crate::network_spec::NetworkSpec;
use crate::types::{Block, BlockTag};

use super::ExecutionRpc;

pub struct HttpRpc<N: NetworkSpec> {
    url: String,
    #[cfg(not(target_arch = "wasm32"))]
    provider: RootProvider<RetryBackoffService<Http<Client>>, N>,
    #[cfg(target_arch = "wasm32")]
    provider: RootProvider<Http<Client>, N>,
}

impl<N: NetworkSpec> Clone for HttpRpc<N> {
    fn clone(&self) -> Self {
        Self::new(&self.url).unwrap()
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec> ExecutionRpc<N> for HttpRpc<N> {
    fn new(rpc: &str) -> Result<Self> {
        #[cfg(not(target_arch = "wasm32"))]
        let client = ClientBuilder::default()
            .layer(RetryBackoffLayer::new(100, 50, 300))
            .http(rpc.parse().unwrap());

        #[cfg(target_arch = "wasm32")]
        let client = ClientBuilder::default().http(rpc.parse().unwrap());

        let provider = ProviderBuilder::new().network::<N>().on_client(client);

        Ok(HttpRpc {
            url: rpc.to_string(),
            provider,
        })
    }

    async fn get_proof(
        &self,
        address: Address,
        slots: &[B256],
        block: u64,
    ) -> Result<EIP1186AccountProofResponse> {
        let proof_response = self
            .provider
            .get_proof(address, slots.to_vec())
            .block_id(block.into())
            .await
            .map_err(|e| RpcError::new("get_proof", e))?;

        Ok(proof_response)
    }

    async fn create_access_list(
        &self,
        tx: &N::TransactionRequest,
        block: BlockTag,
    ) -> Result<AccessList> {
        let block = match block {
            BlockTag::Latest => BlockId::latest(),
            BlockTag::Finalized => BlockId::finalized(),
            BlockTag::Number(num) => BlockId::number(num),
        };

        let list = self
            .provider
            .create_access_list(tx)
            .block_id(block)
            .await
            .map_err(|e| RpcError::new("create_access_list", e))?;

        Ok(list.access_list)
    }

    async fn get_code(&self, address: Address, block: u64) -> Result<Vec<u8>> {
        let code = self
            .provider
            .get_code_at(address)
            .block_id(block.into())
            .await
            .map_err(|e| RpcError::new("get_code", e))?;

        Ok(code.to_vec())
    }

    async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<B256> {
        let tx = self
            .provider
            .send_raw_transaction(bytes)
            .await
            .map_err(|e| RpcError::new("send_raw_transaction", e))?;

        Ok(*tx.tx_hash())
    }

    async fn get_transaction_receipt(&self, tx_hash: B256) -> Result<Option<N::ReceiptResponse>> {
        let receipt = self
            .provider
            .get_transaction_receipt(tx_hash)
            .await
            .map_err(|e| RpcError::new("get_transaction_receipt", e))?;

        Ok(receipt)
    }

    async fn get_block_receipts(&self, block: BlockTag) -> Result<Option<Vec<N::ReceiptResponse>>> {
        let block = match block {
            BlockTag::Latest => BlockNumberOrTag::Latest,
            BlockTag::Finalized => BlockNumberOrTag::Finalized,
            BlockTag::Number(num) => BlockNumberOrTag::Number(num),
        };

        let receipts = self
            .provider
            .get_block_receipts(block)
            .await
            .map_err(|e| RpcError::new("get_block_receipts", e))?;

        Ok(receipts)
    }

    async fn get_transaction(&self, tx_hash: B256) -> Result<Option<N::TransactionResponse>> {
        Ok(self
            .provider
            .get_transaction_by_hash(tx_hash)
            .await
            .map_err(|e| RpcError::new("get_transaction", e))?)
    }

    async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>> {
        Ok(self
            .provider
            .get_logs(filter)
            .await
            .map_err(|e| RpcError::new("get_logs", e))?)
    }

    async fn get_filter_changes(&self, filter_id: U256) -> Result<Vec<Log>> {
        Ok(self
            .provider
            .get_filter_changes(filter_id)
            .await
            .map_err(|e| RpcError::new("get_filter_changes", e))?)
    }

    async fn get_filter_logs(&self, filter_id: U256) -> Result<Vec<Log>> {
        Ok(self
            .provider
            .raw_request("eth_getFilterLogs".into(), (filter_id,))
            .await
            .map_err(|e| RpcError::new("get_filter_logs", e))?)
    }

    async fn uninstall_filter(&self, _filter_id: U256) -> Result<bool> {
        // TODO: support uninstalling
        Ok(true)
    }

    async fn get_new_filter(&self, filter: &Filter) -> Result<U256> {
        Ok(self
            .provider
            .new_filter(filter)
            .await
            .map_err(|e| RpcError::new("get_new_filter", e))?)
    }

    async fn get_new_block_filter(&self) -> Result<U256> {
        Ok(self
            .provider
            .new_block_filter()
            .await
            .map_err(|e| RpcError::new("get_new_block_filter", e))?)
    }

    async fn get_new_pending_transaction_filter(&self) -> Result<U256> {
        Ok(self
            .provider
            .new_pending_transactions_filter(true)
            .await
            .map_err(|e| RpcError::new("get_new_pending_transactions", e))?)
    }

    async fn chain_id(&self) -> Result<u64> {
        Ok(self
            .provider
            .get_chain_id()
            .await
            .map_err(|e| RpcError::new("chain_id", e))?)
    }

    async fn get_fee_history(
        &self,
        block_count: u64,
        last_block: u64,
        reward_percentiles: &[f64],
    ) -> Result<FeeHistory> {
        Ok(self
            .provider
            .get_fee_history(block_count, last_block.into(), reward_percentiles)
            .await
            .map_err(|e| RpcError::new("fee_history", e))?)
    }

    async fn get_block(&self, hash: B256) -> Result<Block<N::TransactionResponse>> {
        self.provider
            .raw_request::<_, Option<Block<N::TransactionResponse>>>(
                "eth_getBlockByHash".into(),
                (hash, true),
            )
            .await?
            .ok_or(eyre!("block not found"))
    }
}
