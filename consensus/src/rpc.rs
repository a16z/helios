use eyre::Result;

use super::types::*;

pub struct Rpc {
    rpc: String,
}

impl Rpc {
    pub fn new(rpc: &str) -> Self {
        Rpc {
            rpc: rpc.to_string(),
        }
    }

    pub async fn get_bootstrap(&self, block_root: &Vec<u8>) -> Result<Bootstrap> {
        let root_hex = hex::encode(block_root);
        let req = format!(
            "{}/eth/v0/beacon/light_client/bootstrap/0x{}",
            self.rpc, root_hex
        );
        let res = reqwest::get(req).await?.json::<BootstrapResponse>().await?;
        Ok(res.data.v)
    }

    pub async fn get_updates(&self, period: u64) -> Result<Vec<Update>> {
        let req = format!(
            "{}/eth/v0/beacon/light_client/updates?start_period={}&count=1000",
            self.rpc, period
        );
        let res = reqwest::get(req).await?.json::<UpdateResponse>().await?;
        Ok(res.data)
    }

    pub async fn get_finality_update(&self) -> Result<FinalityUpdate> {
        let req = format!("{}/eth/v0/beacon/light_client/finality_update", self.rpc);
        let res = reqwest::get(req)
            .await?
            .json::<FinalityUpdateResponse>()
            .await?;
        Ok(res.data)
    }

    pub async fn get_optimistic_update(&self) -> Result<OptimisticUpdate> {
        let req = format!("{}/eth/v0/beacon/light_client/optimistic_update", self.rpc);
        let res = reqwest::get(req)
            .await?
            .json::<OptimisticUpdateResponse>()
            .await?;
        Ok(res.data)
    }

    pub async fn get_block(&self, slot: u64) -> Result<BeaconBlock> {
        let req = format!("{}/eth/v2/beacon/blocks/{}", self.rpc, slot);
        let res = reqwest::get(req)
            .await?
            .json::<BeaconBlockResponse>()
            .await?;
        Ok(res.data.message)
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

#[derive(serde::Deserialize, Debug)]
struct UpdateResponse {
    data: Vec<Update>,
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
    data: BootstrapData,
}

#[derive(serde::Deserialize, Debug)]
struct BootstrapData {
    v: Bootstrap,
}
