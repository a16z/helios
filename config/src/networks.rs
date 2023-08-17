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
}

impl Network {
    pub fn to_base_config(&self) -> BaseConfig {
        match self {
            Self::MAINNET => mainnet(),
            Self::GOERLI => goerli(),
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
            "0x69e3bb0e44b707636ff2c1d3385b20c21013c95f867c296c7ee84960edcbd103",
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
            "0x428474ba0c1ce5049efe4324d503f69f0370fae2095954db4864c57097c90574",
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
