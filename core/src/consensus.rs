use alloy::network::TransactionResponse;
use eyre::Result;
use serde::Serialize;
use tokio::sync::{mpsc, watch};

use crate::common::types::Block;

pub trait Consensus<T: TransactionResponse + Serialize>: Sync + Send + 'static {
    fn block_recv(&mut self) -> Option<mpsc::Receiver<Block<T>>>;
    fn finalized_block_recv(&mut self) -> Option<watch::Receiver<Option<Block<T>>>>;
    fn expected_highest_block(&self) -> u64;
    fn chain_id(&self) -> u64;
    fn shutdown(&self) -> Result<()>;
}
