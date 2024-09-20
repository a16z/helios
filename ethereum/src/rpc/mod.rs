pub mod mock_rpc;
pub mod nimbus_rpc;

use alloy::primitives::B256;
use async_trait::async_trait;
use eyre::Result;

use helios_consensus_core::types::{
    BeaconBlock, Bootstrap, FinalityUpdate, OptimisticUpdate, Update,
};

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait ConsensusRpc: Send + Sync + 'static {
    fn new(path: &str) -> Self;
    async fn get_bootstrap(&self, checkpoint: B256) -> Result<Bootstrap>;
    async fn get_updates(&self, period: u64, count: u8) -> Result<Vec<Update>>;
    async fn get_finality_update(&self) -> Result<FinalityUpdate>;
    async fn get_optimistic_update(&self) -> Result<OptimisticUpdate>;
    async fn get_block(&self, slot: u64) -> Result<BeaconBlock>;
    async fn chain_id(&self) -> Result<u64>;
}
