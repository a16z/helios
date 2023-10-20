use std::str::FromStr;

use common::utils::hex_str_to_bytes;
use eyre::Result;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter};

use crate::base::BaseConfig;
use crate::types::{ChainConfig, Fork, Forks};

#[derive(
    Debug,
    Clone,
    Copy,
    Serialize,
    Deserialize,
    EnumIter,
    Display,
    Hash,
    Eq,
    PartialEq,
    PartialOrd,
    Ord,
)]
pub enum Network {
    MAINNET,
    GOERLI,
    SEPOLIA,
}

impl FromStr for Network {
    type Err = eyre::Report;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "mainnet" => Ok(Self::MAINNET),
            "goerli" => Ok(Self::GOERLI),
            "sepolia" => Ok(Self::SEPOLIA),
            _ => Err(eyre::eyre!("network not recognized")),
        }
    }
}

impl Network {
    pub fn to_base_config(&self) -> BaseConfig {
        match self {
            Self::MAINNET => mainnet(),
            Self::GOERLI => goerli(),
            Self::SEPOLIA => sepolia(),
        }
    }

    pub fn from_chain_id(id: u64) -> Result<Self> {
        match id {
            1 => Ok(Network::MAINNET),
            5 => Ok(Network::GOERLI),
            _ => Err(eyre::eyre!("chain id not known")),
        }
    }
}

pub fn mainnet() -> BaseConfig {
    BaseConfig {
        default_checkpoint: hex_str_to_bytes(
            "0x7e495f50102bed2aae9e698b73a2a640de548658998033d732feb1c0c0c7f80b",
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
            capella: Fork {
                epoch: 194048,
                fork_version: hex_str_to_bytes("0x03000000").unwrap(),
            },
        },
        max_checkpoint_age: 1_209_600, // 14 days
        ..std::default::Default::default()
    }
}

pub fn goerli() -> BaseConfig {
    BaseConfig {
        default_checkpoint: hex_str_to_bytes(
            "0xf6e9d5fdd7c406834e888961beab07b2443b64703c36acc1274ae1ce8bb48839",
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
            capella: Fork {
                epoch: 162304,
                fork_version: hex_str_to_bytes("0x03001020").unwrap(),
            },
        },
        max_checkpoint_age: 1_209_600, // 14 days
        ..std::default::Default::default()
    }
}

pub fn sepolia() -> BaseConfig {
    BaseConfig {
        default_checkpoint: hex_str_to_bytes(
            "0x4135bf01bddcfadac11143ba911f1c7f0772fdd6b87742b0bc229887bbf62b48",
        )
        .unwrap(),
        rpc_port: 8545,
        consensus_rpc: None,
        chain: ChainConfig {
            chain_id: 11155111,
            genesis_time: 1655733600,
            genesis_root: hex_str_to_bytes(
                "0xd8ea171f3c94aea21ebc42a1ed61052acf3f9209c00e4efbaaddac09ed9b8078",
            )
            .unwrap(),
        },
        forks: Forks {
            genesis: Fork {
                epoch: 0,
                fork_version: hex_str_to_bytes("0x90000069").unwrap(),
            },
            altair: Fork {
                epoch: 50,
                fork_version: hex_str_to_bytes("0x90000070").unwrap(),
            },
            bellatrix: Fork {
                epoch: 100,
                fork_version: hex_str_to_bytes("0x90000071").unwrap(),
            },
            capella: Fork {
                epoch: 56832,
                fork_version: hex_str_to_bytes("0x90000072").unwrap(),
            },
        },
        max_checkpoint_age: 1_209_600, // 14 days
        ..std::default::Default::default()
    }
}
