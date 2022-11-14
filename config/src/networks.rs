use serde::Serialize;

use crate::{bytes_serialize, ChainConfig, Fork, Forks};
use common::utils::hex_str_to_bytes;

pub enum Network {
    MAINNET,
    GOERLI,
}

impl Network {
    pub fn to_base_config(&self) -> BaseConfig {
        match self {
            Self::MAINNET => mainnet(),
            Self::GOERLI => goerli(),
        }
    }
}

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

pub fn mainnet() -> BaseConfig {
    BaseConfig {
        checkpoint: hex_str_to_bytes(
            "0x428ce0b5f5bbed1fc2b3feb5d4152ae0fe98a80b1bfa8de36681868e81e9222a",
        )
        .unwrap(),
        rpc_port: 8545,
        consensus_rpc: Some("https://www.lightclientdata.org".to_string()),
        chain: ChainConfig {
            chain_id: 1,
            genesis_time: 1606824023,
            genesis_root: hex_str_to_bytes(
                "0x4b363db94e286120d76eb905340fdd4e54bfe9f06bf33ff6cf5ad27f511bfe95",
            )
            .unwrap(),
        },
        forks: Forks {
            genesis: Fork {
                epoch: 0,
                fork_version: hex_str_to_bytes("0x00000000").unwrap(),
            },
            altair: Fork {
                epoch: 74240,
                fork_version: hex_str_to_bytes("0x01000000").unwrap(),
            },
            bellatrix: Fork {
                epoch: 144896,
                fork_version: hex_str_to_bytes("0x02000000").unwrap(),
            },
        },
        max_checkpoint_age: 1_209_600, // 14 days
    }
}

pub fn goerli() -> BaseConfig {
    BaseConfig {
        checkpoint: hex_str_to_bytes(
            "0xd4344682866dbede543395ecf5adf9443a27f423a4b00f270458e7932686ced1",
        )
        .unwrap(),
        rpc_port: 8545,
        consensus_rpc: None,
        chain: ChainConfig {
            chain_id: 5,
            genesis_time: 1616508000,
            genesis_root: hex_str_to_bytes(
                "0x043db0d9a83813551ee2f33450d23797757d430911a9320530ad8a0eabc43efb",
            )
            .unwrap(),
        },
        forks: Forks {
            genesis: Fork {
                epoch: 0,
                fork_version: hex_str_to_bytes("0x00001020").unwrap(),
            },
            altair: Fork {
                epoch: 36660,
                fork_version: hex_str_to_bytes("0x01001020").unwrap(),
            },
            bellatrix: Fork {
                epoch: 112260,
                fork_version: hex_str_to_bytes("0x02001020").unwrap(),
            },
        },
        max_checkpoint_age: 1_209_600, // 14 days
    }
}
