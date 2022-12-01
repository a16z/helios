pub mod networks;

use std::{collections::HashMap, path::PathBuf, process::exit};

use eyre::Result;
use figment::{
    providers::{Format, Serialized, Toml},
    value::Value,
    Figment,
};
use networks::BaseConfig;
use serde::{Deserialize, Serialize};

use common::utils::hex_str_to_bytes;

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Config {
    pub consensus_rpc: String,
    pub execution_rpc: String,
    pub rpc_port: Option<u16>,
    #[serde(
        deserialize_with = "bytes_deserialize",
        serialize_with = "bytes_serialize"
    )]
    pub checkpoint: Vec<u8>,
    pub data_dir: Option<PathBuf>,
    pub chain: ChainConfig,
    pub forks: Forks,
    pub max_checkpoint_age: u64,
    pub with_ws: bool,
    pub with_http: bool,
}

impl Config {
    pub fn from_file(config_path: &PathBuf, network: &str, cli_config: &CliConfig) -> Self {
        let base_config = match network {
            "mainnet" => networks::mainnet(),
            "goerli" => networks::goerli(),
            _ => BaseConfig::default(),
        };

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

                        println!(
                            "\x1b[91merror\x1b[0m: missing configuration field: {}",
                            field
                        );

                        println!(
                            "\n\ttry supplying the propoper command line argument: --{}",
                            field
                        );

                        println!("\talternatively, you can add the field to your helios.toml file or as an environment variable");
                        println!("\nfor more information, check the github README");
                    }
                    _ => println!("cannot parse configuration: {}", err),
                }
                exit(1);
            }
        }
    }

    pub fn fork_version(&self, slot: u64) -> Vec<u8> {
        let epoch = slot / 32;

        if epoch >= self.forks.bellatrix.epoch {
            self.forks.bellatrix.fork_version.clone()
        } else if epoch >= self.forks.altair.epoch {
            self.forks.altair.fork_version.clone()
        } else {
            self.forks.genesis.fork_version.clone()
        }
    }

    pub fn to_base_config(&self) -> BaseConfig {
        BaseConfig {
            rpc_port: self.rpc_port.unwrap_or(8545),
            consensus_rpc: Some(self.consensus_rpc.clone()),
            checkpoint: self.checkpoint.clone(),
            chain: self.chain.clone(),
            forks: self.forks.clone(),
            max_checkpoint_age: self.max_checkpoint_age,
        }
    }
}

#[derive(Serialize)]
pub struct CliConfig {
    pub execution_rpc: Option<String>,
    pub consensus_rpc: Option<String>,
    pub checkpoint: Option<Vec<u8>>,
    pub rpc_port: Option<u16>,
    pub data_dir: PathBuf,
}

impl CliConfig {
    fn as_provider(&self, network: &str) -> Serialized<HashMap<&str, Value>> {
        let mut user_dict = HashMap::new();

        if let Some(rpc) = &self.execution_rpc {
            user_dict.insert("execution_rpc", Value::from(rpc.clone()));
        }

        if let Some(rpc) = &self.consensus_rpc {
            user_dict.insert("consensus_rpc", Value::from(rpc.clone()));
        }

        if let Some(checkpoint) = &self.checkpoint {
            user_dict.insert("checkpoint", Value::from(hex::encode(checkpoint)));
        }

        if let Some(port) = self.rpc_port {
            user_dict.insert("rpc_port", Value::from(port));
        }

        user_dict.insert("data_dir", Value::from(self.data_dir.to_str().unwrap()));

        Serialized::from(user_dict, network)
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct ChainConfig {
    pub chain_id: u64,
    pub genesis_time: u64,
    #[serde(
        deserialize_with = "bytes_deserialize",
        serialize_with = "bytes_serialize"
    )]
    pub genesis_root: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Forks {
    pub genesis: Fork,
    pub altair: Fork,
    pub bellatrix: Fork,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Fork {
    pub epoch: u64,
    #[serde(
        deserialize_with = "bytes_deserialize",
        serialize_with = "bytes_serialize"
    )]
    pub fork_version: Vec<u8>,
}

fn bytes_deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let bytes: String = serde::Deserialize::deserialize(deserializer)?;
    Ok(hex_str_to_bytes(&bytes).unwrap())
}

fn bytes_serialize<S>(bytes: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let bytes_string = hex::encode(bytes);
    serializer.serialize_str(&bytes_string)
}
