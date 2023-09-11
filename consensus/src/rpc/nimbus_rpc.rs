use async_trait::async_trait;
use eyre::Result;
use log::warn;
use std::cmp;

use super::ConsensusRpc;
use crate::constants::MAX_REQUEST_LIGHT_CLIENT_UPDATES;
use crate::types::*;
use backoff::ExponentialBackoff;
use backoff::future::retry_notify;
use common::errors::RpcError;

#[derive(Debug)]
pub struct NimbusRpc {
    rpc: String,
}

async fn get(req: &str) -> Result<reqwest::Response, reqwest::Error> {
    retry_notify(
        ExponentialBackoff::default(),
        || async {
            Ok(reqwest::get(req).await?)
        },
        |e, dur| {
            warn!("rpc error occurred at {:?}: {}", dur, e)
        }
    ).await
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

        let client = reqwest::Client::new();
        let res = client
            .get(req)   // TODO implement backoff
            .send()
            .await
            .map_err(|e| RpcError::new("bootstrap", e))?
            .json::<BootstrapResponse>()
            .await
            .map_err(|e| RpcError::new("bootstrap", e))?;

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
            .get(req)   // TODO implement backoff
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
        let res = get(&req).await?.json::<FinalityUpdateResponse>().await?;

        Ok(res.data)
    }

    async fn get_optimistic_update(&self) -> Result<OptimisticUpdate> {
        let req = format!("{}/eth/v1/beacon/light_client/optimistic_update", self.rpc);
        let res = retry_notify(
            ExponentialBackoff::default(),
            || async {
                Ok(reqwest::get(req.as_str()).await?.json::<OptimisticUpdateResponse>().await?)
            },
            |e, dur| {
                warn!("optimistic_update rpc error occurred at {:?}: {}", dur, e)
            }
        ).await?;

        Ok(res.data)
    }

    async fn get_block(&self, slot: u64) -> Result<BeaconBlock> {
        let req = format!("{}/eth/v2/beacon/blocks/{}", self.rpc, slot);
        let res = retry_notify(
            ExponentialBackoff::default(),
            || async {
                Ok(reqwest::get(req.as_str()).await?.json::<BeaconBlockResponse>().await?)
            },
            |e, dur| {
                warn!("block rpc error occurred at {:?}: {}", dur, e)
            }
        ).await?;

        Ok(res.data.message)
    }

    async fn chain_id(&self) -> Result<u64> {
        let req = format!("{}/eth/v1/config/spec", self.rpc);
        let res = retry_notify(
            ExponentialBackoff::default(),
            || async {
                Ok(reqwest::get(req.as_str()).await?.json::<SpecResponse>().await?)
            },
            |e, dur| {
                warn!("chain id rpc error occurred at {:?}: {}", dur, e)
            }
        ).await?;

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
