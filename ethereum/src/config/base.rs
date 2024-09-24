use std::default::Default;
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;

use alloy::primitives::B256;
use serde::Serialize;

use helios_consensus_core::types::Forks;

use crate::config::types::ChainConfig;

/// The base configuration for a network.
#[derive(Serialize)]
pub struct BaseConfig {
    pub rpc_bind_ip: IpAddr,
    pub rpc_port: u16,
    pub consensus_rpc: Option<String>,
    pub default_checkpoint: B256,
    pub chain: ChainConfig,
    pub forks: Forks,
    pub max_checkpoint_age: u64,
    pub data_dir: Option<PathBuf>,
    pub load_external_fallback: bool,
    pub strict_checkpoint_age: bool,
}

impl Default for BaseConfig {
    fn default() -> Self {
        BaseConfig {
            rpc_bind_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
            rpc_port: 0,
            consensus_rpc: None,
            default_checkpoint: B256::ZERO,
            chain: Default::default(),
            forks: Default::default(),
            max_checkpoint_age: 0,
            data_dir: None,
            load_external_fallback: false,
            strict_checkpoint_age: false,
        }
    }
}
