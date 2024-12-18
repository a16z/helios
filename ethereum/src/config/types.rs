use alloy::primitives::B256;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct ChainConfig {
    pub chain_id: u64,
    pub genesis_time: u64,
    pub genesis_root: B256,
    pub block_time: u64,
}
