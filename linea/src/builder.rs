use eyre::{eyre, Result};
use helios_core::execution::providers::block::block_cache::BlockCache;
use helios_core::execution::providers::rpc::RpcExecutionProvider;
#[cfg(not(target_arch = "wasm32"))]
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use helios_common::fork_schedule::ForkSchedule;

use crate::config::{Config, Network};
use crate::consensus::ConsensusClient;
use crate::spec::Linea;
use crate::LineaClient;

#[derive(Default)]
pub struct LineaClientBuilder {
    network: Option<Network>,
    execution_rpc: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    rpc_bind_ip: Option<IpAddr>,
    #[cfg(not(target_arch = "wasm32"))]
    rpc_port: Option<u16>,
    config: Option<Config>,
}

impl LineaClientBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn network(mut self, network: Network) -> Self {
        self.network = Some(network);
        self
    }

    pub fn execution_rpc(mut self, execution_rpc: &str) -> Self {
        self.execution_rpc = Some(execution_rpc.to_string());
        self
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn rpc_bind_ip(mut self, ip: IpAddr) -> Self {
        self.rpc_bind_ip = Some(ip);
        self
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn rpc_port(mut self, port: u16) -> Self {
        self.rpc_port = Some(port);
        self
    }

    pub fn config(mut self, config: Config) -> Self {
        self.config = Some(config);
        self
    }

    pub fn build(self) -> Result<LineaClient> {
        let base_config = if let Some(network) = self.network {
            network.to_base_config()
        } else {
            let config = self
                .config
                .as_ref()
                .ok_or(eyre!("missing network config"))?;
            config.to_base_config()
        };

        let execution_rpc = self.execution_rpc.unwrap_or_else(|| {
            self.config
                .as_ref()
                .expect("missing execution rpc")
                .execution_rpc
                .clone()
        });

        #[cfg(not(target_arch = "wasm32"))]
        let rpc_bind_ip = if self.rpc_bind_ip.is_some() {
            self.rpc_bind_ip
        } else if let Some(config) = &self.config {
            config.rpc_bind_ip
        } else {
            Some(base_config.rpc_bind_ip)
        };

        #[cfg(not(target_arch = "wasm32"))]
        let rpc_port = if self.rpc_port.is_some() {
            self.rpc_port
        } else if let Some(config) = &self.config {
            config.rpc_port
        } else {
            None
        };

        let config = Config {
            execution_rpc,
            #[cfg(not(target_arch = "wasm32"))]
            rpc_bind_ip,
            #[cfg(target_arch = "wasm32")]
            rpc_bind_ip: None,
            #[cfg(not(target_arch = "wasm32"))]
            rpc_port,
            #[cfg(target_arch = "wasm32")]
            rpc_port: None,
            chain: base_config.chain,
        };

        #[cfg(not(target_arch = "wasm32"))]
        let socket = if let (Some(ip), Some(port)) = (rpc_bind_ip, rpc_port) {
            Some(SocketAddr::new(ip, port))
        } else {
            None
        };

        let config = Arc::new(config);
        let consensus = ConsensusClient::new(&config);

        let fork_schedule = ForkSchedule {
            prague_timestamp: u64::MAX,
        };

        let block_provider = BlockCache::<Linea>::new();
        let execution =
            RpcExecutionProvider::new(config.execution_rpc.parse().unwrap(), block_provider);

        Ok(LineaClient::new(
            consensus,
            execution,
            fork_schedule,
            #[cfg(not(target_arch = "wasm32"))]
            socket,
        ))
    }
}
