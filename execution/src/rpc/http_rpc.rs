use std::str::FromStr;

use async_trait::async_trait;
use common::errors::RpcError;
use ethers::prelude::{Address, Http};
use ethers::providers::{HttpRateLimitRetryPolicy, Middleware, Provider, RetryClient};
use ethers::types::transaction::eip2718::TypedTransaction;
use ethers::types::transaction::eip2930::AccessList;
use ethers::types::{
    BlockId, Bytes, EIP1186ProofResponse, Eip1559TransactionRequest, Transaction,
    TransactionReceipt, H256, U256,
};
use eyre::Result;
use log::trace;

use crate::types::CallOpts;

use super::Rpc;

pub struct HttpRpc {
    url: String,
    provider: Provider<RetryClient<Http>>,
}

impl Clone for HttpRpc {
    fn clone(&self) -> Self {
        Self::new(&self.url).unwrap()
    }
}

#[async_trait]
impl Rpc for HttpRpc {
    fn new(rpc: &str) -> Result<Self> {
        let http = Http::from_str(rpc)?;
        let mut client = RetryClient::new(http, Box::new(HttpRateLimitRetryPolicy), 100, 250);
        client.set_compute_units(100);
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
        trace!("fetching proof");
        let block = Some(BlockId::from(block));
        let proof_response = self
            .provider
            .get_proof(*address, slots.to_vec(), block)
            .await
            .map_err(|e| RpcError::new(e.to_string()))?;

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
            .map_err(|e| RpcError::new(e.to_string()))?;

        Ok(list.access_list)
    }

    async fn get_code(&self, address: &Address, block: u64) -> Result<Vec<u8>> {
        let block = Some(BlockId::from(block));
        let code = self
            .provider
            .get_code(*address, block)
            .await
            .map_err(|e| RpcError::new(e.to_string()))?;

        Ok(code.to_vec())
    }

    async fn send_raw_transaction(&self, bytes: &Vec<u8>) -> Result<H256> {
        let bytes = Bytes::from(bytes.as_slice().to_owned());
        let tx = self
            .provider
            .send_raw_transaction(bytes)
            .await
            .map_err(|e| RpcError::new(e.to_string()))?;

        Ok(tx.tx_hash())
    }

    async fn get_transaction_receipt(&self, tx_hash: &H256) -> Result<Option<TransactionReceipt>> {
        let receipt = self
            .provider
            .get_transaction_receipt(*tx_hash)
            .await
            .map_err(|e| RpcError::new(e.to_string()))?;

        Ok(receipt)
    }

    async fn get_transaction(&self, tx_hash: &H256) -> Result<Option<Transaction>> {
        Ok(self.provider.get_transaction(*tx_hash).await?)
    }
}
