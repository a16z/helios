use eyre::Result;
use helios_core::execution::providers::{
    block::block_cache::BlockCache, historical::eip2935::Eip2935Provider,
    rpc::RpcExecutionProvider, verifiable_api::VerifiableApiExecutionProvider,
};
use reqwest::{IntoUrl, Url};
use std::net::SocketAddr;

use crate::{
    config::{Config, Network, NetworkConfig},
    consensus::ConsensusClient,
    spec::OpStack,
    OpStackClient,
};

#[derive(Default)]
pub struct OpStackClientBuilder {
    config: Option<Config>,
    network: Option<Network>,
    consensus_rpc: Option<Url>,
    execution_rpc: Option<String>,
    verifiable_api: Option<String>,
    rpc_socket: Option<SocketAddr>,
    verify_unsafe_signer: Option<bool>,
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

    pub fn verifiable_api(mut self, verifiable_api: &str) -> Self {
        self.verifiable_api = Some(verifiable_api.to_string());
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

    pub fn verify_unsafe_signer(mut self, value: bool) -> Self {
        self.verify_unsafe_signer = Some(value);
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
                verifiable_api: self.verifiable_api,
                rpc_socket: self.rpc_socket,
                chain: NetworkConfig::from(network).chain,
                load_external_fallback: None,
                checkpoint: None,
                verify_unsafe_signer: self.verify_unsafe_signer.unwrap_or_default(),
            }
        };

        let consensus = ConsensusClient::new(&config);

        if let Some(verifiable_api) = &config.verifiable_api {
            let block_provider = BlockCache::<OpStack>::new();
            // Create EIP-2935 historical block provider
            let historical_provider = Eip2935Provider::new();
            let execution = VerifiableApiExecutionProvider::with_historical_provider(
                verifiable_api,
                block_provider,
                historical_provider,
            );

            Ok(OpStackClient::new(
                consensus,
                execution,
                config.chain.forks,
                #[cfg(not(target_arch = "wasm32"))]
                config.rpc_socket,
            ))
        } else {
            let block_provider = BlockCache::<OpStack>::new();
            // Create EIP-2935 historical block provider
            let rpc_url: Url = config.execution_rpc.as_ref().unwrap().parse().unwrap();
            let historical_provider = Eip2935Provider::new();
            let execution = RpcExecutionProvider::with_historical_provider(
                rpc_url,
                block_provider,
                historical_provider,
            );

            Ok(OpStackClient::new(
                consensus,
                execution,
                config.chain.forks,
                #[cfg(not(target_arch = "wasm32"))]
                config.rpc_socket,
            ))
        }
    }
}
