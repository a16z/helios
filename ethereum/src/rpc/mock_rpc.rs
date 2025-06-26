use std::{
    fs::read_to_string,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use alloy::primitives::B256;
use async_trait::async_trait;
use eyre::Result;

use helios_consensus_core::{
    consensus_spec::ConsensusSpec,
    types::{BeaconBlock, Bootstrap, FinalityUpdate, OptimisticUpdate, Update},
};
use serde::Deserialize;

use super::ConsensusRpc;

pub struct MockRpc {
    testdata: PathBuf,
    pub fetched_updates: Arc<Mutex<bool>>,
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S: ConsensusSpec> ConsensusRpc<S> for MockRpc {
    fn new(path: &str) -> Self {
        // Handle file:// URLs by extracting the path
        let testdata = if let Some(stripped) = path.strip_prefix("file://") {
            PathBuf::from(stripped)
        } else {
            PathBuf::from(path)
        };

        MockRpc {
            testdata,
            fetched_updates: Arc::new(Mutex::new(false)),
        }
    }

    async fn get_bootstrap(&self, _block_root: B256) -> Result<Bootstrap<S>> {
        let res = read_to_string(self.testdata.join("bootstrap.json"))?;
        let bootstrap: BootstrapResponse<S> = serde_json::from_str(&res)?;
        Ok(bootstrap.data)
    }

    async fn get_updates(&self, _period: u64, _count: u8) -> Result<Vec<Update<S>>> {
        if *self.fetched_updates.lock().unwrap() {
            return Ok(Vec::new());
        }
        *self.fetched_updates.lock().unwrap() = true;
        let res = read_to_string(self.testdata.join("updates.json"))?;
        let updates: Vec<UpdateData<S>> = serde_json::from_str(&res)?;
        Ok(updates.into_iter().map(|update| update.data).collect())
    }

    async fn get_finality_update(&self) -> Result<FinalityUpdate<S>> {
        let res = read_to_string(self.testdata.join("finality.json"))?;
        let finality: FinalityUpdateResponse<S> = serde_json::from_str(&res)?;
        Ok(finality.data)
    }

    async fn get_optimistic_update(&self) -> Result<OptimisticUpdate<S>> {
        let res = read_to_string(self.testdata.join("optimistic.json"))?;
        let optimistic: OptimisticUpdateResponse<S> = serde_json::from_str(&res)?;
        Ok(optimistic.data)
    }

    async fn get_block(&self, slot: u64) -> Result<BeaconBlock<S>> {
        let path = self.testdata.join(format!("blocks/{slot}.json"));
        let res = read_to_string(path)?;
        let block: BeaconBlockResponse<S> = serde_json::from_str(&res)?;
        Ok(block.data.message)
    }

    async fn chain_id(&self) -> Result<u64> {
        eyre::bail!("not implemented")
    }
}

#[derive(Deserialize, Debug)]
#[serde(bound = "S: ConsensusSpec")]
struct BeaconBlockResponse<S: ConsensusSpec> {
    data: BeaconBlockData<S>,
}

#[derive(Deserialize, Debug)]
#[serde(bound = "S: ConsensusSpec")]
struct BeaconBlockData<S: ConsensusSpec> {
    message: BeaconBlock<S>,
}

#[derive(Deserialize, Debug)]
#[serde(bound = "S: ConsensusSpec")]
struct UpdateData<S: ConsensusSpec> {
    data: Update<S>,
}

#[derive(Deserialize, Debug)]
#[serde(bound = "S: ConsensusSpec")]
struct FinalityUpdateResponse<S: ConsensusSpec> {
    data: FinalityUpdate<S>,
}

#[derive(Deserialize, Debug)]
#[serde(bound = "S: ConsensusSpec")]
struct OptimisticUpdateResponse<S: ConsensusSpec> {
    data: OptimisticUpdate<S>,
}

#[derive(Deserialize, Debug)]
#[serde(bound = "S: ConsensusSpec")]
struct BootstrapResponse<S: ConsensusSpec> {
    data: Bootstrap<S>,
}
