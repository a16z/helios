use alloy::network::{primitives::HeaderResponse, BlockResponse, TransactionResponse};
use async_trait::async_trait;
use eyre::Result;
use serde::Serialize;
use tokio::sync::{mpsc, watch};

#[async_trait]
pub trait Consensus<
    B: BlockResponse<Transaction: TransactionResponse, Header: HeaderResponse> + Serialize,
>: Sync + Send + 'static
{
    fn block_recv(&mut self) -> Option<mpsc::Receiver<B>>;
    fn finalized_block_recv(&mut self) -> Option<watch::Receiver<Option<B>>>;
    fn expected_highest_block(&self) -> u64;
    fn chain_id(&self) -> u64;
    fn shutdown(&self) -> Result<()>;
    async fn wait_synced(&self) -> Result<()>;
}
