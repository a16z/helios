use std::marker::PhantomData;
#[cfg(not(target_arch = "wasm32"))]
use std::net::SocketAddr;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
use std::sync::Arc;

use alloy::primitives::B256;
use eyre::{eyre, Result};
use reqwest::{IntoUrl, Url};

use helios_consensus_core::consensus_spec::MainnetConsensusSpec;
use helios_core::execution::providers::block::block_cache::BlockCache;
use helios_core::execution::providers::historical::eip2935::Eip2935Provider;
use helios_core::execution::providers::rpc::RpcExecutionProvider;
use helios_core::execution::providers::verifiable_api::VerifiableApiExecutionProvider;

use crate::config::networks::Network;
use crate::config::Config;
use crate::consensus::ConsensusClient;
#[cfg(not(target_arch = "wasm32"))]
use crate::database::FileDB;
use crate::database::{ConfigDB, Database};
use crate::rpc::http_rpc::HttpRpc;
use crate::spec::Ethereum;
use crate::EthereumClient;

pub struct EthereumClientBuilder<DB: Database> {
    network: Option<Network>,
    consensus_rpc: Option<Url>,
    execution_rpc: Option<Url>,
    verifiable_api: Option<Url>,
    checkpoint: Option<B256>,
    #[cfg(not(target_arch = "wasm32"))]
    rpc_address: Option<SocketAddr>,
    #[cfg(not(target_arch = "wasm32"))]
    data_dir: Option<PathBuf>,
    config: Option<Config>,
    fallback: Option<Url>,
    load_external_fallback: bool,
    strict_checkpoint_age: bool,
    phantom: PhantomData<DB>,
}

impl<DB: Database> Default for EthereumClientBuilder<DB> {
    fn default() -> Self {
        Self {
            network: None,
            consensus_rpc: None,
            execution_rpc: None,
            verifiable_api: None,
            checkpoint: None,
            #[cfg(not(target_arch = "wasm32"))]
            rpc_address: None,
            #[cfg(not(target_arch = "wasm32"))]
            data_dir: None,
            config: None,
            fallback: None,
            load_external_fallback: false,
            strict_checkpoint_age: false,
            phantom: PhantomData,
        }
    }
}

impl<DB: Database> EthereumClientBuilder<DB> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn network(mut self, network: Network) -> Self {
        self.network = Some(network);
        self
    }

    pub fn consensus_rpc<T: IntoUrl>(mut self, consensus_rpc: T) -> Result<Self> {
        self.consensus_rpc = Some(
            consensus_rpc
                .into_url()
                .map_err(|_| eyre!("Invalid consensus RPC URL"))?,
        );
        Ok(self)
    }

    pub fn execution_rpc<T: IntoUrl>(mut self, execution_rpc: T) -> Result<Self> {
        self.execution_rpc = Some(
            execution_rpc
                .into_url()
                .map_err(|_| eyre!("Invalid execution RPC URL"))?,
        );
        Ok(self)
    }

    pub fn verifiable_api<T: IntoUrl>(mut self, verifiable_api: T) -> Result<Self> {
        self.verifiable_api = Some(
            verifiable_api
                .into_url()
                .map_err(|_| eyre!("Invalid verifiable API URL"))?,
        );
        Ok(self)
    }

    pub fn checkpoint(mut self, checkpoint: B256) -> Self {
        self.checkpoint = Some(checkpoint);
        self
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn rpc_address(mut self, rpc_address: SocketAddr) -> Self {
        self.rpc_address = Some(rpc_address);
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

    pub fn fallback<T: IntoUrl>(mut self, fallback: T) -> Result<Self> {
        self.fallback = Some(
            fallback
                .into_url()
                .map_err(|_| eyre!("Invalid fallback URL"))?,
        );
        Ok(self)
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

        let verifiable_api = self
            .verifiable_api
            .or_else(|| self.config.as_ref().and_then(|c| c.verifiable_api.clone()));

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

        #[cfg(not(target_arch = "wasm32"))]
        let rpc_address = if let Some(addr) = self.rpc_address {
            Some(addr)
        } else if let Some(config) = &self.config {
            config
                .rpc_bind_ip
                .zip(config.rpc_port)
                .map(|(addr, port)| SocketAddr::new(addr, port))
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

        let database_type = if let Some(config) = &self.config {
            config.database_type.clone()
        } else {
            None
        };

        let config = Config {
            consensus_rpc,
            execution_rpc,
            verifiable_api,
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
            database_type,
        };

        let config = Arc::new(config);
        let consensus = ConsensusClient::<MainnetConsensusSpec, HttpRpc, DB>::new(
            &config.consensus_rpc,
            config.clone(),
        )?;

        if let Some(verifiable_api) = &config.verifiable_api {
            let block_provider = BlockCache::<Ethereum>::new();
            // Create EIP-2935 historical block provider
            let historical_provider = Eip2935Provider::new();
            let execution = VerifiableApiExecutionProvider::with_historical_provider(
                verifiable_api,
                block_provider,
                historical_provider,
            );
            Ok(EthereumClient::new(
                consensus,
                execution,
                config.execution_forks,
                #[cfg(not(target_arch = "wasm32"))]
                rpc_address,
            ))
        } else {
            let block_provider = BlockCache::<Ethereum>::new();
            // Create EIP-2935 historical block provider
            let rpc_url = config.execution_rpc.as_ref().unwrap().clone();
            let historical_provider = Eip2935Provider::new();
            let execution = RpcExecutionProvider::with_historical_provider(
                rpc_url,
                block_provider,
                historical_provider,
            );

            Ok(EthereumClient::new(
                consensus,
                execution,
                config.execution_forks,
                #[cfg(not(target_arch = "wasm32"))]
                rpc_address,
            ))
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl EthereumClientBuilder<FileDB> {
    pub fn with_file_db(self) -> Self {
        self
    }
}

impl EthereumClientBuilder<ConfigDB> {
    pub fn with_config_db(self) -> Self {
        self
    }
}
