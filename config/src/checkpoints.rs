use std::collections::HashMap;

use ethers::types::H256;
use serde::{Deserialize, Serialize};

/// The location where the list of checkpoint services are stored.
pub const CHECKPOINT_SYNC_SERVICES_LIST: &str = "https://raw.githubusercontent.com/ethpandaops/checkpoint-sync-health-checks/master/_data/endpoints.yaml";

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawSlotResponse {
    pub data: RawSlotResponseData,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawSlotResponseData {
    pub slots: Vec<Slot>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Slot {
    pub slot: u64,
    pub block_root: Option<H256>,
    pub state_root: Option<H256>,
    pub epoch: u64,
    pub time: StartEndTime,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartEndTime {
    /// An ISO 8601 formatted UTC timestamp.
    pub start_time: String,
    /// An ISO 8601 formatted UTC timestamp.
    pub end_time: String,
}

/// A health check for the checkpoint sync service.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Health {
    /// If the node is healthy.
    pub result: bool,
    /// An [ISO 8601](https://en.wikipedia.org/wiki/ISO_8601) UTC timestamp.
    pub date: String,
}

/// A checkpoint fallback service.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointFallback {
    /// The endpoint for the checkpoint sync service.
    pub endpoint: String,
    /// The checkpoint sync service name.
    pub name: String,
    /// The service state.
    pub state: bool,
    /// If the service is verified.
    pub verification: bool,
    /// Contact information for the service maintainers.
    pub contacts: Option<serde_yaml::Value>,
    /// Service Notes
    pub notes: Option<serde_yaml::Value>,
    /// The service health check.
    pub health: Vec<Health>,
}

/// The CheckpointFallbackList is a list of checkpoint fallback services.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointFallbackList {
    /// The list of mainnet checkpoint fallback services.
    pub mainnet: Vec<CheckpointFallback>,
    /// The list of goerli checkpoint fallback services.
    pub goerli: Vec<CheckpointFallback>,
}

impl CheckpointFallbackList {
    /// Constructs a new checkpoint fallback service.
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct the checkpoint fallback service from the community-maintained list by [ethPandaOps](https://github.com/ethpandaops).
    ///
    /// The list is defined in [ethPandaOps/checkpoint-fallback-service](https://github.com/ethpandaops/checkpoint-sync-health-checks/blob/master/_data/endpoints.yaml).
    pub async fn construct(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();
        let res = client.get(CHECKPOINT_SYNC_SERVICES_LIST).send().await?;

        let yaml = res.text().await?;

        // Parse the yaml content results.
        let list: CheckpointFallbackList = serde_yaml::from_str(&yaml)?;
        self.mainnet = list.mainnet;
        self.goerli = list.goerli;
        Ok(())
    }

    /// Fetch the latests mainnet checkpoint sync service checkpoint.
    pub async fn fetch_latest_mainnet_checkpoint(
        &self,
    ) -> Result<H256, Box<dyn std::error::Error>> {
        Self::fetch_latest_checkpoint(&self.mainnet).await
    }

    /// Fetch the latests goerli checkpoint sync service checkpoint.
    pub async fn fetch_latest_goerli_checkpoint(&self) -> Result<H256, Box<dyn std::error::Error>> {
        Self::fetch_latest_checkpoint(&self.goerli).await
    }

    /// Fetch the latest checkpoint sync service health check.
    pub async fn fetch_latest_checkpoint(
        services: &[CheckpointFallback],
    ) -> Result<H256, Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();
        let mut slots = Vec::new();

        // Iterate over all mainnet checkpoint sync services and get the latest checkpoint slot for each.
        // TODO: We can execute this in parallel and collect results into a slots vector.
        for service in services.iter() {
            let constructed_url = Self::construct_url(&service.endpoint);
            let res = client.get(&constructed_url).send().await;
            if res.is_err() {
                continue;
            }
            let raw: Result<RawSlotResponse, _> = res?.json().await;
            if raw.is_err() {
                continue;
            }
            let raw_slot_response = raw?;
            if raw_slot_response.data.slots.is_empty() {
                continue;
            }
            slots.push(raw_slot_response.data.slots[0].clone());
        }

        // Get the max epoch
        let max_epoch_slot = slots
            .iter()
            .max_by_key(|x| x.epoch)
            .ok_or("Failed to find max epoch from checkpoint slots")?;
        let max_epoch = max_epoch_slot.epoch;

        // Filter out all the slots that are not the max epoch.
        let slots = slots
            .into_iter()
            .filter(|x| x.epoch == max_epoch)
            .collect::<Vec<_>>();

        // Return the most commonly verified checkpoint.
        let checkpoints = slots
            .iter()
            .filter_map(|x| x.block_root)
            .collect::<Vec<_>>();
        let mut m: HashMap<H256, usize> = HashMap::new();
        for c in checkpoints {
            *m.entry(c).or_default() += 1;
        }
        let most_common = m.into_iter().max_by_key(|(_, v)| *v).map(|(k, _)| k);

        // Return the most commonly verified checkpoint for the latest epoch.
        most_common.ok_or_else(|| "No checkpoint found".into())
    }

    /// Associated function to fetch the latest checkpoint from a specific checkpoint sync fallback
    /// service api url.
    pub async fn fetch_checkpoint_from_api(url: &str) -> Result<H256, Box<dyn std::error::Error>> {
        // Fetch the url
        let client = reqwest::Client::new();
        let constructed_url = Self::construct_url(url);
        let res = client.get(constructed_url).send().await?;
        let raw: RawSlotResponse = res.json().await?;
        let slot = raw.data.slots[0].clone();
        slot.block_root
            .ok_or_else(|| "Checkpoint not in returned slot".into())
    }

    /// Associated function to construct the checkpoint fallback service url.
    pub fn construct_url(endpoint: &str) -> String {
        format!("{}/checkpointz/v1/beacon/slots", endpoint)
    }

    /// Returns a list of mainnet checkpoint fallback urls.
    pub fn mainnet_urls_unsafe(&self) -> Vec<String> {
        self.mainnet
            .iter()
            .map(|service| service.endpoint.clone())
            .collect()
    }

    /// Returns a list of healthchecked mainnet checkpoint fallback urls.
    pub fn mainnet_urls(&self) -> Vec<String> {
        self.mainnet
            .iter()
            .filter(|service| service.state)
            .map(|service| service.endpoint.clone())
            .collect()
    }

    /// Returns a list of goerli checkpoint fallback urls.
    pub fn goerli_urls_unsafe(&self) -> Vec<String> {
        self.goerli
            .iter()
            .map(|service| service.endpoint.clone())
            .collect()
    }

    /// Returns a list of healthchecked goerli checkpoint fallback urls.
    pub fn goerli_urls(&self) -> Vec<String> {
        self.goerli
            .iter()
            .filter(|service| service.state)
            .map(|service| service.endpoint.clone())
            .collect()
    }
}
