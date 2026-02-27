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
    spec::Mantle,
    MantleClient,
};

#[derive(Default)]
pub struct MantleClientBuilder {
    config: Option<Config>,
    network: Option<Network>,
    consensus_rpc: Option<Url>,
    execution_rpc: Option<Url>,
    verifiable_api: Option<Url>,
    rpc_socket: Option<SocketAddr>,
    verify_unsafe_signer: Option<bool>,
}

impl MantleClientBuilder {
    pub fn new() -> Self {
        MantleClientBuilder::default()
    }

    pub fn config(mut self, config: Config) -> Self {
        self.config = Some(config);
        self
    }

    pub fn consensus_rpc<T: IntoUrl>(mut self, consensus_rpc: T) -> Self {
        self.consensus_rpc = Some(consensus_rpc.into_url().unwrap());
        self
    }

    pub fn execution_rpc<T: IntoUrl>(mut self, execution_rpc: T) -> Self {
        self.execution_rpc = Some(execution_rpc.into_url().unwrap());
        self
    }

    pub fn verifiable_api<T: IntoUrl>(mut self, verifiable_api: T) -> Self {
        self.verifiable_api = Some(verifiable_api.into_url().unwrap());
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

    pub fn build(self) -> Result<MantleClient> {
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
            let block_provider = BlockCache::<Mantle>::new();
            // Create EIP-2935 historical block provider
            let historical_provider = Eip2935Provider::new();
            let execution = VerifiableApiExecutionProvider::with_historical_provider(
                verifiable_api,
                block_provider,
                historical_provider,
            );

            Ok(MantleClient::new(
                consensus,
                execution,
                config.chain.forks,
                #[cfg(not(target_arch = "wasm32"))]
                config.rpc_socket,
            ))
        } else {
            let block_provider = BlockCache::<Mantle>::new();
            // Create EIP-2935 historical block provider
            let rpc_url = config.execution_rpc.as_ref().unwrap().clone();
            let historical_provider = Eip2935Provider::new();
            let execution = RpcExecutionProvider::with_historical_provider(
                rpc_url,
                block_provider,
                historical_provider,
            );

            Ok(MantleClient::new(
                consensus,
                execution,
                config.chain.forks,
                #[cfg(not(target_arch = "wasm32"))]
                config.rpc_socket,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_default() {
        let builder = MantleClientBuilder::new();
        assert!(builder.config.is_none());
        assert!(builder.network.is_none());
        assert!(builder.consensus_rpc.is_none());
        assert!(builder.execution_rpc.is_none());
        assert!(builder.verifiable_api.is_none());
        assert!(builder.rpc_socket.is_none());
        assert!(builder.verify_unsafe_signer.is_none());
    }

    #[test]
    fn test_builder_missing_network() {
        let result = MantleClientBuilder::new()
            .consensus_rpc("http://localhost:8080")
            .execution_rpc("http://localhost:8545")
            .build();
        match result {
            Err(e) => assert!(
                e.to_string().contains("network required"),
                "got: {}",
                e
            ),
            Ok(_) => panic!("expected error for missing network"),
        }
    }

    #[test]
    fn test_builder_missing_consensus_rpc() {
        let result = MantleClientBuilder::new()
            .network(Network::MantleMainnet)
            .execution_rpc("http://localhost:8545")
            .build();
        match result {
            Err(e) => assert!(
                e.to_string().contains("consensus rpc required"),
                "got: {}",
                e
            ),
            Ok(_) => panic!("expected error for missing consensus rpc"),
        }
    }

    #[test]
    fn test_builder_fluent_api() {
        let builder = MantleClientBuilder::new()
            .network(Network::MantleMainnet)
            .consensus_rpc("http://consensus:8080")
            .execution_rpc("http://execution:8545")
            .verify_unsafe_signer(true);

        assert!(builder.network.is_some());
        assert!(builder.consensus_rpc.is_some());
        assert!(builder.execution_rpc.is_some());
        assert_eq!(builder.verify_unsafe_signer, Some(true));
    }

    #[test]
    fn test_builder_with_verifiable_api() {
        let builder = MantleClientBuilder::new()
            .network(Network::MantleSepolia)
            .consensus_rpc("http://consensus:8080")
            .verifiable_api("http://verifiable-api:9000");

        assert!(builder.verifiable_api.is_some());
        assert!(builder.execution_rpc.is_none());
    }

    #[test]
    fn test_builder_rpc_socket() {
        let addr: std::net::SocketAddr = "127.0.0.1:8545".parse().unwrap();
        let builder = MantleClientBuilder::new().rpc_socket(addr);
        assert_eq!(builder.rpc_socket, Some(addr));
    }

    #[test]
    fn test_builder_network_variants() {
        // Verify builder accepts both network variants
        let b1 = MantleClientBuilder::new().network(Network::MantleMainnet);
        assert!(matches!(b1.network, Some(Network::MantleMainnet)));

        let b2 = MantleClientBuilder::new().network(Network::MantleSepolia);
        assert!(matches!(b2.network, Some(Network::MantleSepolia)));
    }
}
