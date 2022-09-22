use async_trait::async_trait;
use ethers::prelude::{Address, Http};
use ethers::providers::{Middleware, Provider};
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

#[derive(Clone)]
pub struct HttpRpc {
    provider: Provider<Http>,
}

#[async_trait]
impl Rpc for HttpRpc {
    fn new(rpc: &str) -> Result<Self> {
        let provider = Provider::try_from(rpc)?;
        Ok(HttpRpc { provider })
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
            .await?;
        Ok(proof_response)
    }

    async fn create_access_list(&self, opts: &CallOpts, block: u64) -> Result<AccessList> {
        let block = Some(BlockId::from(block));

        let mut tx = Eip1559TransactionRequest::new();
        tx.to = Some(opts.to.into());
        tx.from = opts.from;
        tx.value = opts.value;
        // TODO: better way to set gas limit
        tx.gas = Some(U256::from(10_000_000));
        tx.data = opts
            .data
            .as_ref()
            .map(|data| Bytes::from(data.as_slice().to_owned()));

        let tx = TypedTransaction::Eip1559(tx);
        let list = self.provider.create_access_list(&tx, block).await?;

        Ok(list.access_list)
    }

    async fn get_code(&self, address: &Address, block: u64) -> Result<Vec<u8>> {
        let block = Some(BlockId::from(block));
        let code = self.provider.get_code(*address, block).await?;
        Ok(code.to_vec())
    }

    async fn send_raw_transaction(&self, bytes: &Vec<u8>) -> Result<H256> {
        let bytes = Bytes::from(bytes.as_slice().to_owned());
        Ok(self.provider.send_raw_transaction(bytes).await?.tx_hash())
    }

    async fn get_transaction_receipt(&self, tx_hash: &H256) -> Result<Option<TransactionReceipt>> {
        let receipt = self.provider.get_transaction_receipt(*tx_hash).await?;
        Ok(receipt)
    }

    async fn get_transaction(&self, tx_hash: &H256) -> Result<Option<Transaction>> {
        Ok(self.provider.get_transaction(*tx_hash).await?)
    }
}
