use async_trait::async_trait;
use ethers::types::{Address, EIP1186ProofResponse, TransactionReceipt, H256, Transaction};
use eyre::Result;

pub mod http_rpc;

#[async_trait]
pub trait Rpc: Send + Clone + 'static {

    fn new(rpc: &str) -> Result<Self> where Self: Sized;

    async fn get_proof(
        &self,
        address: &Address,
        slots: &[H256],
        block: u64,
    ) -> Result<EIP1186ProofResponse>;

    async fn get_code(&self, address: &Address, block: u64) -> Result<Vec<u8>>;
    async fn send_raw_transaction(&self, bytes: &Vec<u8>) -> Result<H256>;
    async fn get_transaction_receipt(&self, tx_hash: &H256) -> Result<Option<TransactionReceipt>>;
    async fn get_transaction(&self, tx_hash: &H256) -> Result<Option<Transaction>>;
}
