use std::collections::HashMap;

use ethers::types::H256;
use serde::{Deserialize, Serialize};

use crate::networks;

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
pub struct CheckpointFallbackService {
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

/// The CheckpointFallback manages checkpoint fallback services.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointFallback {
    /// Services Map
    pub services: HashMap<networks::Network, Vec<CheckpointFallbackService>>,
    /// A list of supported networks to build.
    /// Default: [mainnet, goerli]
    pub networks: Vec<networks::Network>,
}

impl CheckpointFallback {
    /// Constructs a new checkpoint fallback service.
    pub fn new() -> Self {
        Self {
            services: Default::default(),
            networks: [networks::Network::MAINNET, networks::Network::GOERLI].to_vec(),
        }
    }

    /// Build the checkpoint fallback service from the community-maintained list by [ethPandaOps](https://github.com/ethpandaops).
    ///
    /// The list is defined in [ethPandaOps/checkpoint-fallback-service](https://github.com/ethpandaops/checkpoint-sync-health-checks/blob/master/_data/endpoints.yaml).
    pub async fn build(mut self) -> eyre::Result<Self> {
        // Fetch the services
        let client = reqwest::Client::new();
        let res = client.get(CHECKPOINT_SYNC_SERVICES_LIST).send().await?;
        let yaml = res.text().await?;

        // Parse the yaml content results.
        let list: serde_yaml::Value = serde_yaml::from_str(&yaml)?;

        // Construct the services mapping from network <> list of services
        let mut services = HashMap::new();
        for network in &self.networks {
            // Try to parse list of checkpoint fallback services
            let service_list = list
                .get(network.to_string().to_lowercase())
                .ok_or_else(|| {
                    eyre::eyre!(format!("missing {network} fallback checkpoint services"))
                })?;
            let parsed: Vec<CheckpointFallbackService> =
                serde_yaml::from_value(service_list.clone())?;
            services.insert(*network, parsed);
        }
        self.services = services;

        Ok(self)
    }

    /// Fetch the latest checkpoint from the checkpoint fallback service.
    pub async fn fetch_latest_checkpoint(
        &self,
        network: &crate::networks::Network,
    ) -> eyre::Result<H256> {
        let services = &self.get_healthy_fallback_services(network);
        Self::fetch_latest_checkpoint_from_services(&services[..]).await
    }

    async fn query_service(endpoint: &str) -> Option<RawSlotResponse> {
        let client = reqwest::Client::new();
        let constructed_url = Self::construct_url(endpoint);
        let res = client.get(&constructed_url).send().await.ok()?;
        let raw: RawSlotResponse = res.json().await.ok()?;
        Some(raw)
    }

    /// Fetch the latest checkpoint from a list of checkpoint fallback services.
    pub async fn fetch_latest_checkpoint_from_services(
        services: &[CheckpointFallbackService],
    ) -> eyre::Result<H256> {
        // Iterate over all mainnet checkpoint sync services and get the latest checkpoint slot for each.
        let tasks: Vec<_> = services
            .iter()
            .map(|service| {
                let service = service.clone();
                tokio::spawn(async move {
                    match Self::query_service(&service.endpoint).await {
                        Some(raw) => {
                            if raw.data.slots.is_empty() {
                                return Err(eyre::eyre!("no slots"));
                            }
                            Ok(raw.data.slots[0].clone())
                        }
                        None => Err(eyre::eyre!("failed to query service")),
                    }
                })
            })
            .collect();
        let slots = futures::future::join_all(tasks)
            .await
            .iter()
            .filter_map(|slot| match &slot {
                Ok(Ok(s)) => Some(s.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();

        // Get the max epoch
        let max_epoch_slot = slots.iter().max_by_key(|x| x.epoch).ok_or(eyre::eyre!(
            "Failed to find max epoch from checkpoint slots"
        ))?;
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
        most_common.ok_or_else(|| eyre::eyre!("No checkpoint found"))
    }

    /// Associated function to fetch the latest checkpoint from a specific checkpoint sync fallback
    /// service api url.
    pub async fn fetch_checkpoint_from_api(url: &str) -> eyre::Result<H256> {
        // Fetch the url
        let client = reqwest::Client::new();
        let constructed_url = Self::construct_url(url);
        let res = client.get(constructed_url).send().await?;
        let raw: RawSlotResponse = res.json().await?;
        let slot = raw.data.slots[0].clone();
        slot.block_root
            .ok_or_else(|| eyre::eyre!("Checkpoint not in returned slot"))
    }

    /// Constructs the checkpoint fallback service url for fetching a slot.
    ///
    /// This is an associated function and can be used like so:
    ///
    /// ```rust
    /// use config::CheckpointFallback;
    ///
    /// let url = CheckpointFallback::construct_url("https://sync-mainnet.beaconcha.in");
    /// assert_eq!("https://sync-mainnet.beaconcha.in/checkpointz/v1/beacon/slots", url);
    /// ```
    pub fn construct_url(endpoint: &str) -> String {
        format!("{endpoint}/checkpointz/v1/beacon/slots")
    }

    /// Returns a list of all checkpoint fallback endpoints.
    ///
    /// ### Warning
    ///
    /// These services are not healthchecked **nor** trustworthy and may act with malice by returning invalid checkpoints.
    pub fn get_all_fallback_endpoints(&self, network: &networks::Network) -> Vec<String> {
        self.services[network]
            .iter()
            .map(|service| service.endpoint.clone())
            .collect()
    }

    /// Returns a list of healthchecked checkpoint fallback endpoints.
    ///
    /// ### Warning
    ///
    /// These services are not trustworthy and may act with malice by returning invalid checkpoints.
    pub fn get_healthy_fallback_endpoints(&self, network: &networks::Network) -> Vec<String> {
        self.services[network]
            .iter()
            .filter(|service| service.state)
            .map(|service| service.endpoint.clone())
            .collect()
    }

    /// Returns a list of healthchecked checkpoint fallback services.
    ///
    /// ### Warning
    ///
    /// These services are not trustworthy and may act with malice by returning invalid checkpoints.
    pub fn get_healthy_fallback_services(
        &self,
        network: &networks::Network,
    ) -> Vec<CheckpointFallbackService> {
        self.services[network]
            .iter()
            .filter(|service| service.state)
            .cloned()
            .collect::<Vec<CheckpointFallbackService>>()
    }

    /// Returns the raw checkpoint fallback service objects for a given network.
    pub fn get_fallback_services(
        &self,
        network: &networks::Network,
    ) -> &Vec<CheckpointFallbackService> {
        self.services[network].as_ref()
    }
}
