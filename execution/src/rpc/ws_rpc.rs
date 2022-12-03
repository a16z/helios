use async_trait::async_trait;
use common::errors::RpcError;
use ethers::{
    prelude::*,
    types::transaction::{eip2718::TypedTransaction, eip2930::AccessList},
};
use eyre::Result;

use crate::types::CallOpts;

use super::ExecutionRpc;

pub struct WsRpc {
    pub url: String,
    pub provider: Option<Provider<Ws>>,
}

impl Clone for WsRpc {
    fn clone(&self) -> Self {
        Self::new(&self.url).unwrap()
    }
}

#[async_trait]
impl ExecutionRpc for WsRpc {
    fn new(rpc: &str) -> Result<Self> {
        Ok(Self {
            url: rpc.to_string(),
            provider: None::<Provider<Ws>>,
        })
    }

    async fn connect(&mut self) -> Result<()> {
        let provider = Provider::<Ws>::connect(&self.url).await?;
        self.provider = Some(provider);
        Ok(())
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
            .as_ref()
            .ok_or(RpcError::new(
                "get_proof",
                eyre::eyre!("Provider not connected!"),
            ))?
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
            .as_ref()
            .ok_or(RpcError::new(
                "create_access_list",
                eyre::eyre!("Provider not connected!"),
            ))?
            .create_access_list(&tx, block)
            .await
            .map_err(|e| RpcError::new("create_access_list", e))?;

        Ok(list.access_list)
    }

    async fn get_code(&self, address: &Address, block: u64) -> Result<Vec<u8>> {
        let block = Some(BlockId::from(block));
        let code = self
            .provider
            .as_ref()
            .ok_or(RpcError::new(
                "get_code",
                eyre::eyre!("Provider not connected!"),
            ))?
            .get_code(*address, block)
            .await
            .map_err(|e| RpcError::new("get_code", e))?;

        Ok(code.to_vec())
    }

    async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<H256> {
        let bytes = Bytes::from(bytes.to_owned());
        let tx = self
            .provider
            .as_ref()
            .ok_or(RpcError::new(
                "send_raw_transaction",
                eyre::eyre!("Provider not connected!"),
            ))?
            .send_raw_transaction(bytes)
            .await
            .map_err(|e| RpcError::new("send_raw_transaction", e))?;

        Ok(tx.tx_hash())
    }

    async fn get_transaction_receipt(&self, tx_hash: &H256) -> Result<Option<TransactionReceipt>> {
        let receipt = self
            .provider
            .as_ref()
            .ok_or(RpcError::new(
                "get_transaction_receipt",
                eyre::eyre!("Provider not connected!"),
            ))?
            .get_transaction_receipt(*tx_hash)
            .await
            .map_err(|e| RpcError::new("get_transaction_receipt", e))?;

        Ok(receipt)
    }

    async fn get_transaction(&self, tx_hash: &H256) -> Result<Option<Transaction>> {
        Ok(self
            .provider
            .as_ref()
            .ok_or(RpcError::new(
                "get_transaction",
                eyre::eyre!("Provider not connected!"),
            ))?
            .get_transaction(*tx_hash)
            .await
            .map_err(|e| RpcError::new("get_transaction", e))?)
    }

    async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>> {
        Ok(self
            .provider
            .as_ref()
            .ok_or(RpcError::new(
                "get_logs",
                eyre::eyre!("Provider not connected!"),
            ))?
            .get_logs(filter)
            .await
            .map_err(|e| RpcError::new("get_logs", e))?)
    }
}
