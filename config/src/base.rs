use serde::Serialize;
use std::default::Default;
use std::net::{IpAddr, Ipv4Addr};

use crate::types::{ChainConfig, Forks};
use crate::utils::bytes_serialize;

/// The base configuration for a network.
#[derive(Serialize)]
pub struct BaseConfig {
    pub rpc_bind_ip: IpAddr,
    pub rpc_port: u16,
    pub consensus_rpc: Option<String>,
    #[serde(
        deserialize_with = "bytes_deserialize",
        serialize_with = "bytes_serialize"
    )]
    pub default_checkpoint: Vec<u8>,
    pub chain: ChainConfig,
    pub forks: Forks,
    pub max_checkpoint_age: u64,
}

impl Default for BaseConfig {
    fn default() -> Self {
        BaseConfig {
            rpc_bind_ip: IpAddr::V4(Ipv4Addr::LOCALHOST), // Default to "127.0.0.1"
            rpc_port: 0,
            consensus_rpc: None,
            default_checkpoint: vec![],
            chain: Default::default(),
            forks: Default::default(),
            max_checkpoint_age: 0,
        }
    }
}
