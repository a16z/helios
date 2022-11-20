use std::str::FromStr;

use async_trait::async_trait;
use common::errors::RpcError;
use ethers::prelude::{Address, Http};
use ethers::providers::{Middleware, Provider, RetryClient, HttpRateLimitRetryPolicy};
use ethers::types::transaction::eip2718::TypedTransaction;
use ethers::types::transaction::eip2930::AccessList;
use ethers::types::{
    BlockId, Bytes, EIP1186ProofResponse, Eip1559TransactionRequest, Filter, Log, Transaction,
    TransactionReceipt, H256, U256,
};
use eyre::Result;

use crate::types::CallOpts;

use super::ExecutionRpc;

pub struct HttpRpc {
    url: String,
    provider: Provider<RetryClient<Http>>,
}

impl Clone for HttpRpc {
    fn clone(&self) -> Self {
        Self::new(&self.url).unwrap()
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl ExecutionRpc for HttpRpc {
    fn new(rpc: &str) -> Result<Self> {
        let http = Http::from_str(rpc)?;
        let mut client = RetryClient::new(http, Box::new(HttpRateLimitRetryPolicy), 100, 50);
        client.set_compute_units(300);

        let provider = Provider::new(client);

        Ok(HttpRpc {
            url: rpc.to_string(),
            provider,
        })
    }

    async fn get_proof(
        &self,
        address: &Address,
        slots: &[H256],
        block: u64,
    ) -> Result<EIP1186ProofResponse> {
        let block = Some(BlockId::from(block));
        let proof_response = self
            .provider
            .get_proof(*address, slots.to_vec(), block)
            .await
            .map_err(|e| RpcError::new("get_proof", e))?;

        Ok(proof_response)
    }

    async fn create_access_list(&self, opts: &CallOpts, block: u64) -> Result<AccessList> {
        let block = Some(BlockId::from(block));

        let mut raw_tx = Eip1559TransactionRequest::new();
        raw_tx.to = Some(opts.to.into());
        raw_tx.from = opts.from;
        raw_tx.value = opts.value;
        raw_tx.gas = Some(opts.gas.unwrap_or(U256::from(100_000_000)));
        raw_tx.max_fee_per_gas = Some(U256::zero());
        raw_tx.max_priority_fee_per_gas = Some(U256::zero());
        raw_tx.data = opts
            .data
            .as_ref()
            .map(|data| Bytes::from(data.as_slice().to_owned()));

        let tx = TypedTransaction::Eip1559(raw_tx);
        let list = self
            .provider
            .create_access_list(&tx, block)
            .await
            .map_err(|e| RpcError::new("create_access_list", e))?;

        Ok(list.access_list)
    }

    async fn get_code(&self, address: &Address, block: u64) -> Result<Vec<u8>> {
        let block = Some(BlockId::from(block));
        let code = self
            .provider
            .get_code(*address, block)
            .await
            .map_err(|e| RpcError::new("get_code", e))?;

        Ok(code.to_vec())
    }

    async fn send_raw_transaction(&self, bytes: &Vec<u8>) -> Result<H256> {
        let bytes = Bytes::from(bytes.as_slice().to_owned());
        let tx = self
            .provider
            .send_raw_transaction(bytes)
            .await
            .map_err(|e| RpcError::new("send_raw_transaction", e))?;

        Ok(tx.tx_hash())
    }

    async fn get_transaction_receipt(&self, tx_hash: &H256) -> Result<Option<TransactionReceipt>> {
        let receipt = self
            .provider
            .get_transaction_receipt(*tx_hash)
            .await
            .map_err(|e| RpcError::new("get_transaction_receipt", e))?;

        Ok(receipt)
    }

    async fn get_transaction(&self, tx_hash: &H256) -> Result<Option<Transaction>> {
        Ok(self
            .provider
            .get_transaction(*tx_hash)
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
}
