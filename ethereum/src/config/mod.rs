use std::net::{IpAddr, Ipv4Addr};
use std::str::FromStr;
use std::{path::PathBuf, process::exit};

use alloy::primitives::B256;
use figment::{
    providers::{Format, Serialized, Toml},
    Figment,
};
use serde::Deserialize;

use helios_common::fork_schedule::ForkSchedule;
use helios_consensus_core::types::Forks;

use self::base::BaseConfig;
use self::cli::CliConfig;
use self::networks::Network;
use self::types::ChainConfig;

pub mod checkpoints;
pub mod cli;
pub mod networks;

mod base;
mod types;

#[derive(Deserialize, Debug, Default)]
pub struct Config {
    pub consensus_rpc: String,
    pub execution_rpc: Option<String>,
    pub execution_verifiable_api: Option<String>,
    pub rpc_bind_ip: Option<IpAddr>,
    pub rpc_port: Option<u16>,
    pub default_checkpoint: B256,
    pub checkpoint: Option<B256>,
    pub data_dir: Option<PathBuf>,
    pub chain: ChainConfig,
    pub forks: Forks,
    pub execution_forks: ForkSchedule,
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
                        println!("\talternatively, you can add the field to your helios.toml file");
                        println!("\nfor more information, check the github README");
                    }
                    _ => println!("cannot parse configuration: {err}"),
                }
                exit(1);
            }
        }
    }

    pub fn to_base_config(&self) -> BaseConfig {
        BaseConfig {
            rpc_bind_ip: self.rpc_bind_ip.unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST)),
            rpc_port: self.rpc_port.unwrap_or(8545),
            consensus_rpc: Some(self.consensus_rpc.clone()),
            default_checkpoint: self.default_checkpoint,
            chain: self.chain.clone(),
            forks: self.forks.clone(),
            execution_forks: self.execution_forks,
            max_checkpoint_age: self.max_checkpoint_age,
            data_dir: self.data_dir.clone(),
            load_external_fallback: self.load_external_fallback,
            strict_checkpoint_age: self.strict_checkpoint_age,
        }
    }
}

impl From<BaseConfig> for Config {
    fn from(base: BaseConfig) -> Self {
        Config {
            rpc_bind_ip: Some(base.rpc_bind_ip),
            rpc_port: Some(base.rpc_port),
            consensus_rpc: base.consensus_rpc.unwrap_or_default(),
            execution_rpc: None,
            execution_verifiable_api: None,
            checkpoint: None,
            default_checkpoint: base.default_checkpoint,
            chain: base.chain,
            forks: base.forks,
            execution_forks: base.execution_forks,
            max_checkpoint_age: base.max_checkpoint_age,
            data_dir: base.data_dir,
            fallback: None,
            load_external_fallback: base.load_external_fallback,
            strict_checkpoint_age: base.strict_checkpoint_age,
            database_type: None,
        }
    }
}
