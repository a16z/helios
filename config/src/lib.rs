pub mod networks;

use std::{collections::HashMap, path::PathBuf};

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
}

impl Config {
    pub fn from_file(
        file: &PathBuf,
        network: &str,
        execution_rpc: &Option<String>,
        consensus_rpc: &Option<String>,
        checkpoint: Option<Vec<u8>>,
        port: Option<u16>,
        data_dir: &PathBuf,
    ) -> Result<Self> {
        let base_config = match network {
            "mainnet" => networks::mainnet(),
            "goerli" => networks::goerli(),
            _ => BaseConfig::default(),
        };

        let cli_config = CliConfig {
            execution_rpc: execution_rpc.clone(),
            consensus_rpc: consensus_rpc.clone(),
            checkpoint,
            port,
            data_dir: data_dir.clone(),
        };

        let base_provider = Serialized::from(base_config, network);
        let toml_provider = Toml::file(file).nested();
        let user_provider = cli_config.as_provider(network);

        Ok(Figment::new()
            .merge(base_provider)
            .merge(toml_provider)
            .merge(user_provider)
            .select(network)
            .extract()?)
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
}

#[derive(Serialize)]
struct CliConfig {
    execution_rpc: Option<String>,
    consensus_rpc: Option<String>,
    checkpoint: Option<Vec<u8>>,
    port: Option<u16>,
    data_dir: PathBuf,
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

        if let Some(port) = self.port {
            user_dict.insert("port", Value::from(port));
        }

        user_dict.insert("data_dir", Value::from(self.data_dir.to_str().unwrap()));

        Serialized::from(user_dict, network)
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct ChainConfig {
    pub chain_id: u64,
    pub genesis_time: u64,
    #[serde(
        deserialize_with = "bytes_deserialize",
        serialize_with = "bytes_serialize"
    )]
    pub genesis_root: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Forks {
    pub genesis: Fork,
    pub altair: Fork,
    pub bellatrix: Fork,
}

#[derive(Serialize, Deserialize, Debug, Default)]
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
