use std::cmp;

use alloy::primitives::B256;
use async_trait::async_trait;
use eyre::Result;
use retri::{retry, BackoffSettings};
use serde::{de::DeserializeOwned, Deserialize};

use helios_consensus_core::{
    consensus_spec::ConsensusSpec,
    types::{BeaconBlock, Bootstrap, FinalityUpdate, OptimisticUpdate, Update},
};
use helios_core::errors::RpcError;

use super::ConsensusRpc;
use crate::constants::MAX_REQUEST_LIGHT_CLIENT_UPDATES;

#[derive(Debug)]
pub struct HttpRpc {
    rpc: String,
}

async fn get<R: DeserializeOwned>(req: &str) -> Result<R> {
    let response = retry(
        || async { Ok::<_, eyre::Report>(reqwest::get(req).await?) },
        BackoffSettings::default(),
    )
    .await?;

    if !response.status().is_success() {
        let status = response.status();
        let error_body: serde_json::Value = response.json().await?;
        let message = error_body["message"].as_str().unwrap_or("Unknown error");
        return Err(eyre::eyre!("HTTP error {}: {}", status, message));
    }

    let bytes = response.bytes().await?;
    serde_json::from_slice::<R>(&bytes).map_err(|e| eyre::eyre!("Deserialization error: {}", e))
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S: ConsensusSpec> ConsensusRpc<S> for HttpRpc {
    fn new(rpc: &str) -> Self {
        HttpRpc {
            rpc: rpc.to_string(),
        }
    }

    async fn get_bootstrap(&self, block_root: B256) -> Result<Bootstrap<S>> {
        let root_hex = hex::encode(block_root);
        let req = format!(
            "{}/eth/v1/beacon/light_client/bootstrap/0x{}",
            self.rpc, root_hex
        );

        let res: BootstrapResponse<S> =
            get(&req).await.map_err(|e| RpcError::new("bootstrap", e))?;

        Ok(res.data)
    }

    async fn get_updates(&self, period: u64, count: u8) -> Result<Vec<Update<S>>> {
        let count = cmp::min(count, MAX_REQUEST_LIGHT_CLIENT_UPDATES);
        let req = format!(
            "{}/eth/v1/beacon/light_client/updates?start_period={}&count={}",
            self.rpc, period, count
        );

        let res: Vec<UpdateData<S>> = get(&req).await.map_err(|e| RpcError::new("updates", e))?;

        Ok(res.into_iter().map(|d| d.data).collect())
    }

    async fn get_finality_update(&self) -> Result<FinalityUpdate<S>> {
        let req = format!("{}/eth/v1/beacon/light_client/finality_update", self.rpc);
        let res: FinalityUpdateResponse<S> = get(&req)
            .await
            .map_err(|e| RpcError::new("finality_update", e))?;

        Ok(res.data)
    }

    async fn get_optimistic_update(&self) -> Result<OptimisticUpdate<S>> {
        let req = format!("{}/eth/v1/beacon/light_client/optimistic_update", self.rpc);
        let res: OptimisticUpdateResponse<S> = get(&req)
            .await
            .map_err(|e| RpcError::new("optimistic_update", e))?;

        Ok(res.data)
    }

    async fn get_block(&self, slot: u64) -> Result<BeaconBlock<S>> {
        let req = format!("{}/eth/v2/beacon/blocks/{}", self.rpc, slot);
        let res: BeaconBlockResponse<S> =
            get(&req).await.map_err(|e| RpcError::new("blocks", e))?;

        Ok(res.data.message)
    }

    async fn chain_id(&self) -> Result<u64> {
        let req = format!("{}/eth/v1/config/spec", self.rpc);
        let res: SpecResponse = get(&req).await.map_err(|e| RpcError::new("spec", e))?;

        Ok(res.data.chain_id)
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

#[derive(Deserialize, Debug)]
struct SpecResponse {
    data: Spec,
}

#[derive(Deserialize, Debug)]
struct Spec {
    #[serde(rename = "DEPOSIT_NETWORK_ID")]
    chain_id: u64,
}
