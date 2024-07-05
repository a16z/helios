use std::{fs::read_to_string, path::PathBuf};

use super::ConsensusRpc;
use primitives::types::{BeaconBlock, Bootstrap, FinalityUpdate, OptimisticUpdate, Update};
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
        let res = read_to_string(self.testdata.join("bootstrap.json"))?;
        let bootstrap: BootstrapResponse = serde_json::from_str(&res)?;
        Ok(bootstrap.data)
    }

    async fn get_updates(&self, _period: u64, _count: u8) -> Result<Vec<Update>> {
        let res = read_to_string(self.testdata.join("updates.json"))?;
        let updates: UpdateResponse = serde_json::from_str(&res)?;
        Ok(updates.into_iter().map(|update| update.data).collect())
    }

    async fn get_finality_update(&self) -> Result<FinalityUpdate> {
        let res = read_to_string(self.testdata.join("finality.json"))?;
        let finality: FinalityUpdateResponse = serde_json::from_str(&res)?;
        Ok(finality.data)
    }

    async fn get_optimistic_update(&self) -> Result<OptimisticUpdate> {
        let res = read_to_string(self.testdata.join("optimistic.json"))?;
        let optimistic: OptimisticUpdateResponse = serde_json::from_str(&res)?;
        Ok(optimistic.data)
    }

    async fn get_block(&self, slot: u64) -> Result<BeaconBlock> {
        let path = self.testdata.join(format!("blocks/{}.json", slot));
        let res = read_to_string(path)?;
        let block: BeaconBlockResponse = serde_json::from_str(&res)?;
        Ok(block.data.message)
    }

    async fn chain_id(&self) -> Result<u64> {
        eyre::bail!("not implemented")
    }
}

#[derive(serde::Deserialize, Debug)]
struct BeaconBlockResponse {
    data: BeaconBlockData,
}

#[derive(serde::Deserialize, Debug)]
struct BeaconBlockData {
    message: BeaconBlock,
}

type UpdateResponse = Vec<UpdateData>;

#[derive(serde::Deserialize, Debug)]
struct UpdateData {
    data: Update,
}

#[derive(serde::Deserialize, Debug)]
struct FinalityUpdateResponse {
    data: FinalityUpdate,
}

#[derive(serde::Deserialize, Debug)]
struct OptimisticUpdateResponse {
    data: OptimisticUpdate,
}

#[derive(serde::Deserialize, Debug)]
struct BootstrapResponse {
    data: Bootstrap,
}
