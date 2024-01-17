use crate::base::BaseConfig;
use crate::cli::CliConfig;
use crate::types::{ChainConfig, Forks};
use crate::utils::{bytes_deserialize, bytes_opt_deserialize};
use crate::Network;
use figment::{
    providers::{Format, Serialized, Toml},
    Figment,
};
use serde::Deserialize;
use std::net::{IpAddr, Ipv4Addr};
use std::str::FromStr;
use std::{path::PathBuf, process::exit};

#[derive(Deserialize, Debug, Default)]
pub struct Config {
    pub consensus_rpc: String,
    pub execution_rpc: String,
    pub rpc_bind_ip: Option<IpAddr>,
    pub rpc_port: Option<u16>,
    #[serde(deserialize_with = "bytes_deserialize")]
    pub default_checkpoint: Vec<u8>,
    #[serde(default)]
    #[serde(deserialize_with = "bytes_opt_deserialize")]
    pub checkpoint: Option<Vec<u8>>,
    pub data_dir: Option<PathBuf>,
    pub chain: ChainConfig,
    pub forks: Forks,
    pub max_checkpoint_age: u64,
    pub fallback: Option<String>,
    pub load_external_fallback: bool,
    pub strict_checkpoint_age: bool,
    pub database_type: Option<String>,
}

impl Config {
    pub fn from_file(config_path: &PathBuf, network: &str, cli_config: &CliConfig) -> Self {
        let base_config = Network::from_str(network)
            .map(|n| n.to_base_config())
            .unwrap_or(BaseConfig::default());

        let base_provider = Serialized::from(base_config, network);
        let toml_provider = Toml::file(config_path).nested();
        let cli_provider = cli_config.as_provider(network);

        let config_res = Figment::new()
            .merge(base_provider)
            .merge(toml_provider)
            .merge(cli_provider)
            .select(network)
            .extract();

        match config_res {
            Ok(config) => config,
            Err(err) => {
                match err.kind {
                    figment::error::Kind::MissingField(field) => {
                        let field = field.replace('_', "-");

                        println!("\x1b[91merror\x1b[0m: missing configuration field: {field}");

                        println!("\n\ttry supplying the proper command line argument: --{field}");

                        println!("\talternatively, you can add the field to your helios.toml file or as an environment variable");
                        println!("\nfor more information, check the github README");
                    }
                    _ => println!("cannot parse configuration: {err}"),
                }
                exit(1);
            }
        }
    }

    pub fn fork_version(&self, slot: u64) -> Vec<u8> {
        let epoch = slot / 32;

        if epoch >= self.forks.deneb.epoch {
            self.forks.deneb.fork_version.clone()
        } else if epoch >= self.forks.capella.epoch {
            self.forks.capella.fork_version.clone()
        } else if epoch >= self.forks.bellatrix.epoch {
            self.forks.bellatrix.fork_version.clone()
        } else if epoch >= self.forks.altair.epoch {
            self.forks.altair.fork_version.clone()
        } else {
            self.forks.genesis.fork_version.clone()
        }
    }

    pub fn to_base_config(&self) -> BaseConfig {
        BaseConfig {
            rpc_bind_ip: self.rpc_bind_ip.unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST)),
            rpc_port: self.rpc_port.unwrap_or(8545),
            consensus_rpc: Some(self.consensus_rpc.clone()),
            default_checkpoint: self.default_checkpoint.clone(),
            chain: self.chain.clone(),
            forks: self.forks.clone(),
            max_checkpoint_age: self.max_checkpoint_age,
            data_dir: self.data_dir.clone(),
            load_external_fallback: self.load_external_fallback,
            strict_checkpoint_age: self.strict_checkpoint_age,
        }
    }
}
