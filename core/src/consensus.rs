use alloy::network::{primitives::HeaderResponse, BlockResponse, TransactionResponse};
use alloy::primitives::B256;
use async_trait::async_trait;
use eyre::Result;
use serde::Serialize;
use tokio::sync::{mpsc, watch};

#[derive(Debug, Clone, Serialize)]
pub enum TrustedBlockRef<B> {
    Full(B),
    Hash(B256),
}

impl<B> TrustedBlockRef<B>
where
    B: BlockResponse<Header: HeaderResponse>,
{
    pub fn block_hash(&self) -> B256 {
        match self {
            Self::Full(block) => block.header().hash(),
            Self::Hash(block_hash) => *block_hash,
        }
    }
}

#[async_trait]
pub trait Consensus<
    B: BlockResponse<Transaction: TransactionResponse, Header: HeaderResponse> + Serialize,
>: Sync + Send + 'static
{
    fn block_recv(&mut self) -> Option<mpsc::Receiver<TrustedBlockRef<B>>>;
    fn finalized_block_recv(&mut self) -> Option<watch::Receiver<Option<TrustedBlockRef<B>>>>;
    fn checkpoint_recv(&self) -> Option<watch::Receiver<Option<B256>>>;
    fn expected_highest_block(&self) -> u64;
    fn chain_id(&self) -> u64;
    fn shutdown(&self) -> Result<()>;
    async fn wait_synced(&self) -> Result<()>;
}
