use serde::Serialize;

use crate::types::{ChainConfig, Forks};
use crate::utils::bytes_serialize;

/// The base configuration for a network.
#[derive(Serialize, Default)]
pub struct BaseConfig {
    pub rpc_port: u16,
    pub consensus_rpc: Option<String>,
    #[serde(
        deserialize_with = "bytes_deserialize",
        serialize_with = "bytes_serialize"
    )]
    pub checkpoint: Vec<u8>,
    pub chain: ChainConfig,
    pub forks: Forks,
    pub max_checkpoint_age: u64,
}
