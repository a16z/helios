use std::net::SocketAddr;

use alloy::primitives::Address;
use eyre::Result;
use reqwest::{IntoUrl, Url};

use crate::{
    config::{ChainConfig, Config},
    consensus::ConsensusClient,
    OpStackClient,
};

#[derive(Default)]
pub struct OpStackClientBuilder {
    config: Option<Config>,
    chain_id: Option<u64>,
    unsafe_signer: Option<Address>,
    consensus_rpc: Option<Url>,
    execution_rpc: Option<Url>,
    rpc_socket: Option<SocketAddr>,
}

impl OpStackClientBuilder {
    pub fn new() -> Self {
        OpStackClientBuilder::default()
    }

    pub fn config(mut self, config: Config) -> Self {
        self.config = Some(config);
        self
    }

    pub fn chain_id(mut self, chain_id: u64) -> Self {
        self.chain_id = Some(chain_id);
        self
    }

    pub fn unsafe_signer(mut self, signer: Address) -> Self {
        self.unsafe_signer = Some(signer);
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

    pub fn rpc_socket(mut self, socket: SocketAddr) -> Self {
        self.rpc_socket = Some(socket);
        self
    }

    pub fn build(self) -> Result<OpStackClient> {
        let config = if let Some(config) = self.config {
            config
        } else {
            let Some(chain_id) = self.chain_id else {
                eyre::bail!("chain id required");
            };

            let Some(unsafe_signer) = self.unsafe_signer else {
                eyre::bail!("unsafe signer required");
            };

            let Some(consensus_rpc) = self.consensus_rpc else {
                eyre::bail!("consensus rpc required");
            };

            let Some(execution_rpc) = self.execution_rpc else {
                eyre::bail!("execution rpc required");
            };

            Config {
                consensus_rpc,
                execution_rpc,
                rpc_socket: self.rpc_socket,
                chain: ChainConfig {
                    chain_id,
                    unsafe_signer,
                    system_config_contract: Address::ZERO,
                },
            }
        };

        let consensus = ConsensusClient::new(&config);
        OpStackClient::new(
            &config.execution_rpc.to_string(),
            consensus,
            #[cfg(not(target_arch = "wasm32"))]
            config.rpc_socket,
        )
    }
}
