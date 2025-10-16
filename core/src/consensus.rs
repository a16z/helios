use alloy::network::{primitives::HeaderResponse, BlockResponse, TransactionResponse};
use eyre::Result;
use serde::Serialize;
use tokio::sync::{mpsc, watch};

pub trait Consensus<
    B: BlockResponse<Transaction: TransactionResponse, Header: HeaderResponse> + Serialize,
>: Sync + Send + 'static
{
    fn status(&self) -> Result<()>;
    fn block_recv(&mut self) -> Option<mpsc::Receiver<B>>;
    fn finalized_block_recv(&mut self) -> Option<watch::Receiver<Option<B>>>;
    fn expected_highest_block(&self) -> u64;
    fn chain_id(&self) -> u64;
    fn shutdown(&self) -> Result<()>;
}
