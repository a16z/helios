use std::fmt::Display;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
use std::str::FromStr;

use alloy::primitives::{b256, fixed_bytes};
#[cfg(not(target_arch = "wasm32"))]
use dirs::home_dir;
use eyre::Result;
use serde::{Deserialize, Serialize};
use strum::EnumIter;

use helios_common::fork_schedule::ForkSchedule;
use helios_consensus_core::types::{Fork, Forks};

use crate::config::base::BaseConfig;
use crate::config::types::ChainConfig;

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, EnumIter, Hash, Eq, PartialEq, PartialOrd, Ord,
)]
pub enum Network {
    Mainnet,
    Sepolia,
    Holesky,
    Hoodi,
}

impl FromStr for Network {
    type Err = eyre::Report;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "mainnet" => Ok(Self::Mainnet),
            "sepolia" => Ok(Self::Sepolia),
            "holesky" => Ok(Self::Holesky),
            "hoodi" => Ok(Self::Hoodi),
            _ => Err(eyre::eyre!("network not recognized")),
        }
    }
}

impl Display for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            Self::Mainnet => "mainnet",
            Self::Sepolia => "sepolia",
            Self::Holesky => "holesky",
            Self::Hoodi => "hoodi",
        };

        f.write_str(str)
    }
}

impl Network {
    pub fn to_base_config(&self) -> BaseConfig {
        match self {
            Self::Mainnet => mainnet(),
            Self::Sepolia => sepolia(),
            Self::Holesky => holesky(),
            Self::Hoodi => hoodi(),
        }
    }

    pub fn from_chain_id(id: u64) -> Result<Self> {
        match id {
            1 => Ok(Network::Mainnet),
            11155111 => Ok(Network::Sepolia),
            17000 => Ok(Network::Holesky),
            560048 => Ok(Network::Hoodi),
            _ => Err(eyre::eyre!("chain id not known")),
        }
    }
}

pub fn mainnet() -> BaseConfig {
    BaseConfig {
        default_checkpoint: b256!(
            "0xe4163704b79dbb52a91ba6be1ae6f5504b060522f5495c73b6c55865412b428c"
        ),
        rpc_port: 8545,
        consensus_rpc: Some("https://ethereum.operationsolarstorm.org".to_string()),
        chain: ChainConfig {
            chain_id: 1,
            genesis_time: 1606824023,
            genesis_root: b256!("4b363db94e286120d76eb905340fdd4e54bfe9f06bf33ff6cf5ad27f511bfe95"),
        },
        forks: Forks {
            genesis: Fork {
                epoch: 0,
                fork_version: fixed_bytes!("00000000"),
            },
            altair: Fork {
                epoch: 74240,
                fork_version: fixed_bytes!("01000000"),
            },
            bellatrix: Fork {
                epoch: 144896,
                fork_version: fixed_bytes!("02000000"),
            },
            capella: Fork {
                epoch: 194048,
                fork_version: fixed_bytes!("03000000"),
            },
            deneb: Fork {
                epoch: 269568,
                fork_version: fixed_bytes!("04000000"),
            },
            electra: Fork {
                epoch: 364032,
                fork_version: fixed_bytes!("05000000"),
            },
        },
        execution_forks: ForkSchedule {
            prague_timestamp: 1746612311,
        },
        max_checkpoint_age: 1_209_600, // 14 days
        #[cfg(not(target_arch = "wasm32"))]
        data_dir: Some(data_dir(Network::Mainnet)),
        ..std::default::Default::default()
    }
}

pub fn sepolia() -> BaseConfig {
    BaseConfig {
        default_checkpoint: b256!(
            "234931a3fe5d791f06092477357e2d65dcf6fa6cad048680eb93ad3ea494bbcd"
        ),
        rpc_port: 8545,
        consensus_rpc: None,
        chain: ChainConfig {
            chain_id: 11155111,
            genesis_time: 1655733600,
            genesis_root: b256!("d8ea171f3c94aea21ebc42a1ed61052acf3f9209c00e4efbaaddac09ed9b8078"),
        },
        forks: Forks {
            genesis: Fork {
                epoch: 0,
                fork_version: fixed_bytes!("90000069"),
            },
            altair: Fork {
                epoch: 50,
                fork_version: fixed_bytes!("90000070"),
            },
            bellatrix: Fork {
                epoch: 100,
                fork_version: fixed_bytes!("90000071"),
            },
            capella: Fork {
                epoch: 56832,
                fork_version: fixed_bytes!("90000072"),
            },
            deneb: Fork {
                epoch: 132608,
                fork_version: fixed_bytes!("90000073"),
            },
            electra: Fork {
                epoch: 222464,
                fork_version: fixed_bytes!("90000074"),
            },
        },
        execution_forks: ForkSchedule {
            prague_timestamp: 1741159776,
        },
        max_checkpoint_age: 1_209_600, // 14 days
        #[cfg(not(target_arch = "wasm32"))]
        data_dir: Some(data_dir(Network::Sepolia)),
        ..std::default::Default::default()
    }
}

pub fn holesky() -> BaseConfig {
    BaseConfig {
        default_checkpoint: b256!(
            "bb1f40340606d3b6d6d610b9933b388ddab585fc8898320c29eb771f75c61b48"
        ),
        rpc_port: 8545,
        consensus_rpc: None,
        chain: ChainConfig {
            chain_id: 17000,
            genesis_time: 1695902400,
            genesis_root: b256!("9143aa7c615a7f7115e2b6aac319c03529df8242ae705fba9df39b79c59fa8b1"),
        },
        forks: Forks {
            genesis: Fork {
                epoch: 0,
                fork_version: fixed_bytes!("01017000"),
            },
            altair: Fork {
                epoch: 0,
                fork_version: fixed_bytes!("02017000"),
            },
            bellatrix: Fork {
                epoch: 0,
                fork_version: fixed_bytes!("03017000"),
            },
            capella: Fork {
                epoch: 256,
                fork_version: fixed_bytes!("04017000"),
            },
            deneb: Fork {
                epoch: 29696,
                fork_version: fixed_bytes!("05017000"),
            },
            electra: Fork {
                epoch: 115968,
                fork_version: fixed_bytes!("06017000"),
            },
        },
        execution_forks: ForkSchedule {
            prague_timestamp: 1740434112,
        },
        max_checkpoint_age: 1_209_600, // 14 days
        #[cfg(not(target_arch = "wasm32"))]
        data_dir: Some(data_dir(Network::Holesky)),
        ..std::default::Default::default()
    }
}

pub fn hoodi() -> BaseConfig {
    BaseConfig {
        default_checkpoint: b256!(
            "689dc3d39faf53c360ada45a734139bfb195f96d04416c797bb0c1a46da903ad"
        ),
        rpc_port: 8545,
        consensus_rpc: None,
        chain: ChainConfig {
            chain_id: 560048,
            genesis_time: 1742213400,
            genesis_root: b256!("212f13fc4df078b6cb7db228f1c8307566dcecf900867401a92023d7ba99cb5f"),
        },
        forks: Forks {
            genesis: Fork {
                epoch: 0,
                fork_version: fixed_bytes!("10000910"),
            },
            altair: Fork {
                epoch: 0,
                fork_version: fixed_bytes!("20000910"),
            },
            bellatrix: Fork {
                epoch: 0,
                fork_version: fixed_bytes!("30000910"),
            },
            capella: Fork {
                epoch: 0,
                fork_version: fixed_bytes!("40000910"),
            },
            deneb: Fork {
                epoch: 0,
                fork_version: fixed_bytes!("50000910"),
            },
            electra: Fork {
                epoch: 2048,
                fork_version: fixed_bytes!("60000910"),
            },
        },
        execution_forks: ForkSchedule {
            prague_timestamp: 1742999832,
        },
        max_checkpoint_age: 1_209_600, // 14 days
        #[cfg(not(target_arch = "wasm32"))]
        data_dir: Some(data_dir(Network::Hoodi)),
        ..std::default::Default::default()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn data_dir(network: Network) -> PathBuf {
    home_dir()
        .unwrap()
        .join(format!(".helios/data/{}", network))
}
