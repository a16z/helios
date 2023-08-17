use async_trait::async_trait;
use eyre::Result;
use std::cmp;
use std::collections::HashMap;
use reqwest::redirect;

use super::ConsensusRpc;
use crate::constants::MAX_REQUEST_LIGHT_CLIENT_UPDATES;
use crate::types::*;
use common::errors::RpcError;

#[derive(Debug)]
pub struct NimbusRpc {
    rpc: String,
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
        println!("get_bootstrap req {:?}", req);

        let mut map = HashMap::new();
        map.insert("body", "json");

        let custom = redirect::Policy::custom(|attempt| {
            if attempt.previous().len() > 5 {
                attempt.error("too many redirects")
            } else if attempt.url().host_str() == Some("example.domain") {
                // prevent redirects to 'example.domain'
                attempt.stop()
            } else {
                attempt.follow()
            }
        });
        let client = reqwest::Client::builder()
            .redirect(custom)
            // disable proxies https://docs.rs/reqwest/0.11.13/reqwest/index.html
            .no_proxy()
            .build()?;
        // let client = reqwest::Client::new();

        let res = client
            .get(req)
            .json(&map)
            .send()
            .await
            .map_err(|e| RpcError::new("bootstrap", e))?
            .json::<BootstrapResponse>()
            .await
            .map_err(|e| RpcError::new("bootstrap", e))?;
        println!("get_bootstrap res {:?}", res);
        Ok(res.data)
    }

    async fn get_updates(&self, period: u64, count: u8) -> Result<Vec<Update>> {
        let count = cmp::min(count, MAX_REQUEST_LIGHT_CLIENT_UPDATES);
        let req = format!(
            "{}/eth/v1/beacon/light_client/updates?start_period={}&count={}",
            self.rpc, period, count
        );

        let client = reqwest::Client::new();
        let res = client
            .get(req)
            .send()
            .await
            .map_err(|e| RpcError::new("updates", e))?
            .json::<UpdateResponse>()
            .await
            .map_err(|e| RpcError::new("updates", e))?;

        Ok(res.into_iter().map(|d| d.data).collect())
    }

    async fn get_finality_update(&self) -> Result<FinalityUpdate> {
        let req = format!("{}/eth/v1/beacon/light_client/finality_update", self.rpc);
        let res = reqwest::get(req)
            .await
            .map_err(|e| RpcError::new("finality_update", e))?
            .json::<FinalityUpdateResponse>()
            .await
            .map_err(|e| RpcError::new("finality_update", e))?;

        Ok(res.data)
    }

    async fn get_optimistic_update(&self) -> Result<OptimisticUpdate> {
        let req = format!("{}/eth/v1/beacon/light_client/optimistic_update", self.rpc);
        let res = reqwest::get(req)
            .await
            .map_err(|e| RpcError::new("optimistic_update", e))?
            .json::<OptimisticUpdateResponse>()
            .await
            .map_err(|e| RpcError::new("optimistic_update", e))?;

        Ok(res.data)
    }

    async fn get_block(&self, slot: u64) -> Result<BeaconBlock> {
        let req = format!("{}/eth/v2/beacon/blocks/{}", self.rpc, slot);
        let res = reqwest::get(req)
            .await
            .map_err(|e| RpcError::new("blocks", e))?
            .json::<BeaconBlockResponse>()
            .await
            .map_err(|e| RpcError::new("blocks", e))?;

        Ok(res.data.message)
    }

    async fn chain_id(&self) -> Result<u64> {
        let req = format!("{}/eth/v1/config/spec", self.rpc);
        let res = reqwest::get(req)
            .await
            .map_err(|e| RpcError::new("spec", e))?
            .json::<SpecResponse>()
            .await
            .map_err(|e| RpcError::new("spec", e))?;

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
    chain_id: primitives::U64,
}
