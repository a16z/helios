use async_trait::async_trait;
use primitives::common::types::BlockTag;
use ethers::types::{
    transaction::eip2930::AccessList, Address, EIP1186ProofResponse, FeeHistory, Filter, Log,
    Transaction, TransactionReceipt, H256, U256,
};
use eyre::Result;

use crate::types::CallOpts;

pub mod http_rpc;
pub mod mock_rpc;

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
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

    async fn create_access_list(&self, opts: &CallOpts, block: BlockTag) -> Result<AccessList>;
    async fn get_code(&self, address: &Address, block: u64) -> Result<Vec<u8>>;
    async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<H256>;
    async fn get_transaction_receipt(&self, tx_hash: &H256) -> Result<Option<TransactionReceipt>>;
    async fn get_transaction(&self, tx_hash: &H256) -> Result<Option<Transaction>>;
    async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>>;
    async fn get_filter_changes(&self, filter_id: &U256) -> Result<Vec<Log>>;
    async fn uninstall_filter(&self, filter_id: &U256) -> Result<bool>;
    async fn get_new_filter(&self, filter: &Filter) -> Result<U256>;
    async fn get_new_block_filter(&self) -> Result<U256>;
    async fn get_new_pending_transaction_filter(&self) -> Result<U256>;
    async fn chain_id(&self) -> Result<u64>;
    async fn get_fee_history(
        &self,
        block_count: u64,
        last_block: u64,
        reward_percentiles: &[f64],
    ) -> Result<FeeHistory>;
}
