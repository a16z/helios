pub mod networks;

use std::{fs, path::Path};

use eyre::Result;
use serde::Deserialize;

use common::utils::hex_str_to_bytes;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub general: General,
    pub forks: Forks,
}

#[derive(Deserialize, Debug)]
pub struct General {
    pub chain_id: u64,
    pub genesis_time: u64,
    #[serde(deserialize_with = "bytes_deserialize")]
    pub genesis_root: Vec<u8>,
    #[serde(deserialize_with = "bytes_deserialize")]
    pub checkpoint: Vec<u8>,
    pub consensus_rpc: String,
    pub execution_rpc: String,
    pub rpc_port: Option<u16>,
}

#[derive(Deserialize, Debug)]
pub struct Forks {
    pub genesis: Fork,
    pub altair: Fork,
    pub bellatrix: Fork,
}

#[derive(Deserialize, Debug)]
pub struct Fork {
    pub epoch: u64,
    #[serde(deserialize_with = "bytes_deserialize")]
    pub fork_version: Vec<u8>,
}

impl Config {
    pub fn from_file(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)?;
        Ok(toml::from_str(&contents)?)
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

fn bytes_deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let bytes: String = serde::Deserialize::deserialize(deserializer)?;
    Ok(hex_str_to_bytes(&bytes).unwrap())
}
