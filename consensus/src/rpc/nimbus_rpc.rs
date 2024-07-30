use std::cmp;

use async_trait::async_trait;
use eyre::Result;
use retri::{retry, BackoffSettings};
use serde::de::DeserializeOwned;

use super::ConsensusRpc;
use crate::constants::MAX_REQUEST_LIGHT_CLIENT_UPDATES;
use common::errors::RpcError;
use consensus_core::types::U64;
use consensus_core::types::{BeaconBlock, Bootstrap, FinalityUpdate, OptimisticUpdate, Update};

#[derive(Debug)]
pub struct NimbusRpc {
    rpc: String,
}

async fn get<R: DeserializeOwned>(req: &str) -> Result<R> {
    let bytes = retry(
        || async { Ok::<_, eyre::Report>(reqwest::get(req).await?.bytes().await?) },
        BackoffSettings::default(),
    )
    .await?;

    Ok(serde_json::from_slice::<R>(&bytes)?)
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl ConsensusRpc for NimbusRpc {
    fn new(rpc: &str) -> Self {
        NimbusRpc {
            rpc: rpc.to_string(),
        }
    }

    async fn get_bootstrap(&self, block_root: &'_ [u8]) -> Result<Bootstrap> {
        let root_hex = hex::encode(block_root);
        let req = format!(
            "{}/eth/v1/beacon/light_client/bootstrap/0x{}",
            self.rpc, root_hex
        );

        let res: BootstrapResponse = get(&req).await.map_err(|e| RpcError::new("bootstrap", e))?;

        Ok(res.data)
    }

    async fn get_updates(&self, period: u64, count: u8) -> Result<Vec<Update>> {
        let count = cmp::min(count, MAX_REQUEST_LIGHT_CLIENT_UPDATES);
        let req = format!(
            "{}/eth/v1/beacon/light_client/updates?start_period={}&count={}",
            self.rpc, period, count
        );

        let res: UpdateResponse = get(&req).await.map_err(|e| RpcError::new("updates", e))?;

        Ok(res.into_iter().map(|d| d.data).collect())
    }

    async fn get_finality_update(&self) -> Result<FinalityUpdate> {
        let req = format!("{}/eth/v1/beacon/light_client/finality_update", self.rpc);
        let res: FinalityUpdateResponse = get(&req)
            .await
            .map_err(|e| RpcError::new("finality_update", e))?;

        Ok(res.data)
    }

    async fn get_optimistic_update(&self) -> Result<OptimisticUpdate> {
        let req = format!("{}/eth/v1/beacon/light_client/optimistic_update", self.rpc);
        let res: OptimisticUpdateResponse = get(&req)
            .await
            .map_err(|e| RpcError::new("optimistic_update", e))?;

        Ok(res.data)
    }

    async fn get_block(&self, slot: u64) -> Result<BeaconBlock> {
        let req = format!("{}/eth/v2/beacon/blocks/{}", self.rpc, slot);
        let res: BeaconBlockResponse = get(&req).await.map_err(|e| RpcError::new("blocks", e))?;

        Ok(res.data.message)
    }

    async fn chain_id(&self) -> Result<u64> {
        let req = format!("{}/eth/v1/config/spec", self.rpc);
        let res: SpecResponse = get(&req).await.map_err(|e| RpcError::new("spec", e))?;

        Ok(res.data.chain_id.into())
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

#[derive(serde::Deserialize, Debug)]
struct SpecResponse {
    data: Spec,
}

#[derive(serde::Deserialize, Debug)]
struct Spec {
    #[serde(rename = "DEPOSIT_NETWORK_ID")]
    chain_id: U64,
}
