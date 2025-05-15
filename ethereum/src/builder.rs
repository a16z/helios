#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
use std::sync::Arc;

use alloy::primitives::B256;
use eyre::{eyre, Result};

use helios_consensus_core::consensus_spec::MainnetConsensusSpec;
use helios_core::execution::providers::block::block_cache::BlockCache;
use helios_core::execution::providers::rpc::RpcExecutionProvider;
use helios_core::execution::providers::verifiable_api::VerifiableApiExecutionProvider;

use crate::config::networks::Network;
use crate::config::Config;
use crate::consensus::ConsensusClient;
use crate::database::FileDB;
use crate::rpc::http_rpc::HttpRpc;
use crate::spec::Ethereum;
use crate::EthereumClient;

#[derive(Default)]
pub struct EthereumClientBuilder {
    network: Option<Network>,
    consensus_rpc: Option<String>,
    execution_rpc: Option<String>,
    execution_verifiable_api: Option<String>,
    checkpoint: Option<B256>,
    #[cfg(not(target_arch = "wasm32"))]
    data_dir: Option<PathBuf>,
    config: Option<Config>,
    fallback: Option<String>,
    load_external_fallback: bool,
    strict_checkpoint_age: bool,
}

impl EthereumClientBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn network(mut self, network: Network) -> Self {
        self.network = Some(network);
        self
    }

    pub fn consensus_rpc(mut self, consensus_rpc: &str) -> Self {
        self.consensus_rpc = Some(consensus_rpc.to_string());
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

    pub fn checkpoint(mut self, checkpoint: B256) -> Self {
        self.checkpoint = Some(checkpoint);
        self
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn data_dir(mut self, data_dir: PathBuf) -> Self {
        self.data_dir = Some(data_dir);
        self
    }

    pub fn config(mut self, config: Config) -> Self {
        self.config = Some(config);
        self
    }

    pub fn fallback(mut self, fallback: &str) -> Self {
        self.fallback = Some(fallback.to_string());
        self
    }

    pub fn load_external_fallback(mut self) -> Self {
        self.load_external_fallback = true;
        self
    }

    pub fn strict_checkpoint_age(mut self) -> Self {
        self.strict_checkpoint_age = true;
        self
    }

    pub fn build(self) -> Result<EthereumClient> {
        let base_config = if let Some(network) = self.network {
            network.to_base_config()
        } else {
            let config = self
                .config
                .as_ref()
                .ok_or(eyre!("missing network config"))?;
            config.to_base_config()
        };

        let consensus_rpc = self.consensus_rpc.unwrap_or_else(|| {
            self.config
                .as_ref()
                .expect("missing consensus rpc")
                .consensus_rpc
                .clone()
        });

        let execution_rpc = self
            .execution_rpc
            .or_else(|| self.config.as_ref().and_then(|c| c.execution_rpc.clone()));

        let execution_verifiable_api = self.execution_verifiable_api.or_else(|| {
            self.config
                .as_ref()
                .and_then(|c| c.execution_verifiable_api.clone())
        });

        let checkpoint = if let Some(checkpoint) = self.checkpoint {
            Some(checkpoint)
        } else if let Some(config) = &self.config {
            config.checkpoint
        } else {
            None
        };

        let default_checkpoint = if let Some(config) = &self.config {
            config.default_checkpoint
        } else {
            base_config.default_checkpoint
        };

        #[cfg(not(target_arch = "wasm32"))]
        let data_dir = if self.data_dir.is_some() {
            self.data_dir
        } else if let Some(config) = &self.config {
            config.data_dir.clone()
        } else {
            None
        };

        let fallback = if self.fallback.is_some() {
            self.fallback
        } else if let Some(config) = &self.config {
            config.fallback.clone()
        } else {
            None
        };

        let load_external_fallback = if let Some(config) = &self.config {
            self.load_external_fallback || config.load_external_fallback
        } else {
            self.load_external_fallback
        };

        let strict_checkpoint_age = if let Some(config) = &self.config {
            self.strict_checkpoint_age || config.strict_checkpoint_age
        } else {
            self.strict_checkpoint_age
        };

        let config = Config {
            consensus_rpc,
            execution_rpc,
            execution_verifiable_api,
            checkpoint,
            default_checkpoint,
            rpc_bind_ip: None,
            rpc_port: None,
            #[cfg(not(target_arch = "wasm32"))]
            data_dir,
            #[cfg(target_arch = "wasm32")]
            data_dir: None,
            chain: base_config.chain,
            forks: base_config.forks,
            execution_forks: base_config.execution_forks,
            max_checkpoint_age: base_config.max_checkpoint_age,
            fallback,
            load_external_fallback,
            strict_checkpoint_age,
            database_type: None,
        };

        let config = Arc::new(config);
        let consensus = ConsensusClient::<MainnetConsensusSpec, HttpRpc, FileDB>::new(
            &config.consensus_rpc,
            config.clone(),
        )?;

        let block_provider = BlockCache::<Ethereum>::new();

        // let execution = RpcExecutionProvider::new(
        //     config.execution_rpc.as_ref().unwrap().parse().unwrap(),
        //     block_provider,
        // );

        let execution = VerifiableApiExecutionProvider::new(
            &config.execution_verifiable_api.as_ref().unwrap(),
            block_provider,
        );

        Ok(EthereumClient::new(
            consensus,
            execution,
            config.execution_forks,
        ))
    }
}
