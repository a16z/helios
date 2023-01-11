use crate::types::CallOpts;
use async_trait::async_trait;
use ethers::types::{
    transaction::eip2930::AccessList, Address, EIP1186ProofResponse, Filter, Log, Transaction,
    TransactionReceipt, H256, U256,
};
use eyre::Result;

pub mod http_rpc;
pub mod mock_rpc;

#[async_trait]
pub trait ExecutionRpc: Send + Clone + Sync + 'static {
    fn new(rpc: &str) -> Result<Self>
    where
        Self: Sized;

    async fn get_proof(
        &self,
        address: &Address,
        slots: &[H256],
        block: u64,
    ) -> Result<EIP1186ProofResponse>;

    async fn create_access_list(&self, opts: &CallOpts, block: u64) -> Result<AccessList>;
    async fn get_code(&self, address: &Address, block: u64) -> Result<Vec<u8>>;
    async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<H256>;
    async fn get_transaction_count(&self, address: &Address, block: u64) -> Result<U256>;
    async fn get_transaction_receipt(&self, tx_hash: &H256) -> Result<Option<TransactionReceipt>>;
    async fn get_transaction(&self, tx_hash: &H256) -> Result<Option<Transaction>>;
    async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>>;
}
