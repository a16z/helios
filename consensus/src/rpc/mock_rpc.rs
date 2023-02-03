use std::{fs::read_to_string, path::PathBuf};

use super::ConsensusRpc;
use crate::types::{BeaconBlock, Bootstrap, FinalityUpdate, OptimisticUpdate, Update};
use async_trait::async_trait;
use eyre::Result;
pub struct MockRpc {
    testdata: PathBuf,
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl ConsensusRpc for MockRpc {
    fn new(path: &str) -> Self {
        MockRpc {
            testdata: PathBuf::from(path),
        }
    }

    async fn get_bootstrap(&self, _block_root: &'_ [u8]) -> Result<Bootstrap> {
        let bootstrap = read_to_string(self.testdata.join("bootstrap.json"))?;
        Ok(serde_json::from_str(&bootstrap)?)
    }

    async fn get_updates(&self, _period: u64, _count: u8) -> Result<Vec<Update>> {
        let updates = read_to_string(self.testdata.join("updates.json"))?;
        Ok(serde_json::from_str(&updates)?)
    }

    async fn get_finality_update(&self) -> Result<FinalityUpdate> {
        let finality = read_to_string(self.testdata.join("finality.json"))?;
        Ok(serde_json::from_str(&finality)?)
    }

    async fn get_optimistic_update(&self) -> Result<OptimisticUpdate> {
        let optimistic = read_to_string(self.testdata.join("optimistic.json"))?;
        Ok(serde_json::from_str(&optimistic)?)
    }

    async fn get_block(&self, _slot: u64) -> Result<BeaconBlock> {
        let block = read_to_string(self.testdata.join("blocks.json"))?;
        Ok(serde_json::from_str(&block)?)
    }

    async fn chain_id(&self) -> Result<u64> {
        eyre::bail!("not implemented")
    }
}
