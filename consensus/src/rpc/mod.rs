pub mod mock_rpc;
pub mod nimbus_rpc;

use async_trait::async_trait;
use eyre::Result;

use common::consensus::types::{BeaconBlock, Bootstrap, FinalityUpdate, OptimisticUpdate, Update};

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait ConsensusRpc: Send + Sync {
    fn new(path: &str) -> Self;
    async fn get_bootstrap(&self, block_root: &'_ [u8]) -> Result<Bootstrap>;
    async fn get_updates(&self, period: u64, count: u8) -> Result<Vec<Update>>;
    async fn get_finality_update(&self) -> Result<FinalityUpdate>;
    async fn get_optimistic_update(&self) -> Result<OptimisticUpdate>;
    async fn get_block(&self, slot: u64) -> Result<BeaconBlock>;
    async fn chain_id(&self) -> Result<u64>;
}
