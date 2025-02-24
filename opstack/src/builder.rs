use eyre::Result;
use reqwest::{IntoUrl, Url};
use std::net::SocketAddr;

use helios_common::{execution_mode::ExecutionMode, fork_schedule::ForkSchedule};

use crate::{
    config::Network,
    config::{Config, NetworkConfig},
    consensus::ConsensusClient,
    OpStackClient,
};

#[derive(Default)]
pub struct OpStackClientBuilder {
    config: Option<Config>,
    network: Option<Network>,
    consensus_rpc: Option<Url>,
    execution_rpc: Option<String>,
    execution_verifiable_api: Option<String>,
    rpc_socket: Option<SocketAddr>,
    verify_unsafe_singer: Option<bool>,
}

impl OpStackClientBuilder {
    pub fn new() -> Self {
        OpStackClientBuilder::default()
    }

    pub fn config(mut self, config: Config) -> Self {
        self.config = Some(config);
        self
    }

    pub fn consensus_rpc<T: IntoUrl>(mut self, consensus_rpc: T) -> Self {
        self.consensus_rpc = Some(consensus_rpc.into_url().unwrap());
        self
    }

    pub fn execution_rpc(mut self, execution_rpc: &str) -> Self {
        self.execution_rpc = Some(execution_rpc.to_string());
        self
    }

    pub fn execution_verifiable_api(mut self, execution_verifiable_api: &str) -> Self {
        self.execution_verifiable_api = Some(execution_verifiable_api.to_string());
        self
    }

    pub fn rpc_socket(mut self, socket: SocketAddr) -> Self {
        self.rpc_socket = Some(socket);
        self
    }

    pub fn network(mut self, network: Network) -> Self {
        self.network = Some(network);
        self
    }

    pub fn verify_unsafe_singer(mut self, value: bool) -> Self {
        self.verify_unsafe_singer = Some(value);
        self
    }

    pub fn build(self) -> Result<OpStackClient> {
        let config = if let Some(config) = self.config {
            config
        } else {
            let Some(network) = self.network else {
                eyre::bail!("network required");
            };

            let Some(consensus_rpc) = self.consensus_rpc else {
                eyre::bail!("consensus rpc required");
            };

            Config {
                consensus_rpc,
                execution_rpc: self.execution_rpc,
                execution_verifiable_api: self.execution_verifiable_api,
                rpc_socket: self.rpc_socket,
                chain: NetworkConfig::from(network).chain,
                load_external_fallback: None,
                checkpoint: None,
                verify_unsafe_signer: self.verify_unsafe_singer.unwrap_or_default(),
            }
        };

        let execution_mode = ExecutionMode::from_urls(
            config.execution_rpc.clone(),
            config.execution_verifiable_api.clone(),
        );
        let consensus = ConsensusClient::new(&config);
        let fork_schedule = ForkSchedule {
            prague_timestamp: u64::MAX,
        };

        OpStackClient::new(
            execution_mode,
            consensus,
            fork_schedule,
            #[cfg(not(target_arch = "wasm32"))]
            config.rpc_socket,
        )
    }
}
