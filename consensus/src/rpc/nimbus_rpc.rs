use async_trait::async_trait;
use common::errors::RpcError;
use eyre::Result;

use super::Rpc;
use crate::types::*;

pub struct NimbusRpc {
    rpc: String,
}

#[async_trait]
impl Rpc for NimbusRpc {
    fn new(rpc: &str) -> Self {
        NimbusRpc {
            rpc: rpc.to_string(),
        }
    }

    async fn get_bootstrap(&self, block_root: &Vec<u8>) -> Result<Bootstrap> {
        let root_hex = hex::encode(block_root);
        let req = format!(
            "{}/eth/v0/beacon/light_client/bootstrap/0x{}",
            self.rpc, root_hex
        );

        let res = reqwest::get(req)
            .await
            .map_err(|e| RpcError::new(e.to_string()))?
            .json::<BootstrapResponse>()
            .await
            .map_err(|e| RpcError::new(e.to_string()))?;

        Ok(res.data.v)
    }

    async fn get_updates(&self, period: u64) -> Result<Vec<Update>> {
        let req = format!(
            "{}/eth/v0/beacon/light_client/updates?start_period={}&count=1000",
            self.rpc, period
        );

        let res = reqwest::get(req)
            .await
            .map_err(|e| RpcError::new(e.to_string()))?
            .json::<UpdateResponse>()
            .await
            .map_err(|e| RpcError::new(e.to_string()))?;

        Ok(res.data)
    }

    async fn get_finality_update(&self) -> Result<FinalityUpdate> {
        let req = format!("{}/eth/v0/beacon/light_client/finality_update", self.rpc);
        let res = reqwest::get(req)
            .await
            .map_err(|e| RpcError::new(e.to_string()))?
            .json::<FinalityUpdateResponse>()
            .await
            .map_err(|e| RpcError::new(e.to_string()))?;

        Ok(res.data)
    }

    async fn get_optimistic_update(&self) -> Result<OptimisticUpdate> {
        let req = format!("{}/eth/v0/beacon/light_client/optimistic_update", self.rpc);
        let res = reqwest::get(req)
            .await
            .map_err(|e| RpcError::new(e.to_string()))?
            .json::<OptimisticUpdateResponse>()
            .await
            .map_err(|e| RpcError::new(e.to_string()))?;

        Ok(res.data)
    }

    async fn get_block(&self, slot: u64) -> Result<BeaconBlock> {
        let req = format!("{}/eth/v2/beacon/blocks/{}", self.rpc, slot);
        let res = reqwest::get(req)
            .await
            .map_err(|e| RpcError::new(e.to_string()))?
            .json::<BeaconBlockResponse>()
            .await
            .map_err(|e| RpcError::new(e.to_string()))?;

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
