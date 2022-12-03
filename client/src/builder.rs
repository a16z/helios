use std::path::PathBuf;

use config::{Config, Network};
use execution::rpc::WsRpc;

use crate::{database::FileDB, Client};

#[derive(Default)]
pub struct ClientBuilder {
    pub network: Option<Network>,
    pub consensus_rpc: Option<String>,
    pub execution_rpc: Option<String>,
    pub checkpoint: Option<Vec<u8>>,
    pub rpc_port: Option<u16>,
    pub data_dir: Option<PathBuf>,
    pub config: Option<Config>,
    pub fallback: Option<String>,
    pub load_external_fallback: bool,
    pub with_ws: bool,
    pub with_http: bool,
}

impl ClientBuilder {
    pub fn new() -> Self {
        Self::default().with_http(true)
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

    pub fn checkpoint(mut self, checkpoint: &str) -> Self {
        let checkpoint = hex::decode(checkpoint.strip_prefix("0x").unwrap_or(checkpoint))
            .expect("cannot parse checkpoint");
        self.checkpoint = Some(checkpoint);
        self
    }

    /// Enables the client to serve a websocket connection.
    ///
    /// # Example
    /// ```rust
    /// let mut client_builder = client::ClientBuilder::new().with_ws(true);
    /// assert_eq!(client_builder.with_ws, true);
    /// client_builder = client_builder.with_ws(false);
    /// assert_eq!(client_builder.with_ws, false);
    /// ```
    pub fn with_ws(mut self, option: bool) -> Self {
        self.with_ws = option;
        self
    }

    /// Enables the client to serve an http connection (enabled by default).
    ///
    /// # Example
    /// ```rust
    /// let mut client_builder = client::ClientBuilder::new();
    /// assert_eq!(client_builder.with_http, true);
    /// client_builder = client_builder.with_http(false);
    /// assert_eq!(client_builder.with_http, false);
    /// ```
    pub fn with_http(mut self, option: bool) -> Self {
        self.with_http = option;
        self
    }

    pub fn rpc_port(mut self, port: u16) -> Self {
        self.rpc_port = Some(port);
        self
    }

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

    fn build_base_config(&self) -> eyre::Result<Config> {
        let base_config = if let Some(network) = self.network {
            network.to_base_config()
        } else {
            let config = self
                .config
                .as_ref()
                .ok_or(eyre::eyre!("missing network config"))?;
            config.to_base_config()
        };

        let consensus_rpc = self.consensus_rpc.clone().unwrap_or_else(|| {
            self.config
                .as_ref()
                .expect("missing consensus rpc")
                .consensus_rpc
                .clone()
        });

        let execution_rpc = self.execution_rpc.clone().unwrap_or_else(|| {
            self.config
                .as_ref()
                .expect("missing execution rpc")
                .execution_rpc
                .clone()
        });

        let checkpoint = if let Some(checkpoint) = &self.checkpoint {
            checkpoint.clone()
        } else if let Some(config) = &self.config {
            config.checkpoint.clone()
        } else {
            base_config.checkpoint
        };

        let rpc_port = if self.rpc_port.is_some() {
            self.rpc_port
        } else if let Some(config) = &self.config {
            config.rpc_port
        } else {
            None
        };

        let data_dir = if self.data_dir.is_some() {
            self.data_dir.clone()
        } else if let Some(config) = &self.config {
            config.data_dir.clone()
        } else {
            None
        };

        let fallback = if self.fallback.is_some() {
            self.fallback.clone()
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

        let with_ws = if let Some(config) = &self.config {
            self.with_ws || config.with_ws
        } else {
            self.with_ws
        };

        let with_http = if let Some(config) = &self.config {
            self.with_http || config.with_http
        } else {
            self.with_http
        };

        Ok(Config {
            consensus_rpc,
            execution_rpc,
            checkpoint,
            rpc_port,
            data_dir,
            chain: base_config.chain,
            forks: base_config.forks,
            max_checkpoint_age: base_config.max_checkpoint_age,
            fallback,
            load_external_fallback,
            with_ws,
            with_http,
        })
    }
}

impl ClientBuilder {
    pub fn build(self) -> eyre::Result<Client<FileDB, WsRpc>> {
        let config = self.build_base_config()?;
        Client::new(config)
    }
}
