use std::fmt::Display;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
use std::str::FromStr;

use alloy::primitives::{b256, fixed_bytes};
#[cfg(not(target_arch = "wasm32"))]
use dirs::home_dir;
use eyre::Result;
use helios_core::fork_schedule::ForkSchedule;
use serde::{Deserialize, Serialize};
use strum::EnumIter;

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
    PectraDevnet,
}

impl FromStr for Network {
    type Err = eyre::Report;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "mainnet" => Ok(Self::Mainnet),
            "sepolia" => Ok(Self::Sepolia),
            "holesky" => Ok(Self::Holesky),
            "pectra-devnet" => Ok(Self::PectraDevnet),
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
            Self::PectraDevnet => "pectra-devnet",
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
            Self::PectraDevnet => pectra_devnet(),
        }
    }

    pub fn from_chain_id(id: u64) -> Result<Self> {
        match id {
            1 => Ok(Network::Mainnet),
            11155111 => Ok(Network::Sepolia),
            17000 => Ok(Network::Holesky),
            _ => Err(eyre::eyre!("chain id not known")),
        }
    }
}

pub fn mainnet() -> BaseConfig {
    BaseConfig {
        default_checkpoint: b256!(
            "2f5319c3db189044de938be99a76d12205290b1bfbb280ef7b0f60a40fbf81b8"
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
                epoch: u64::MAX,
                fork_version: fixed_bytes!("05000000"),
            },
        },
        execution_forks: ForkSchedule {
            prague_timestamp: u64::MAX,
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
            "d2450a31dbdec0be8bc09f2eeddc4b58fc77ed2df9158aa16cb1f6b63b90be3e"
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
            "8d7bf5aa9a46ef46ce2cec02e56c8a203d8157211d3599d61196972cf6def229"
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

pub fn pectra_devnet() -> BaseConfig {
    BaseConfig {
        default_checkpoint: b256!(
            "f52e8522f1abc34fa91f4a0c6560cce6f9d557cfec083f1bc325a74c6060df84"
        ),
        rpc_port: 8545,
        consensus_rpc: None,
        chain: ChainConfig {
            chain_id: 7072151312,
            genesis_time: 1738603860,
            genesis_root: b256!("5c074f81fbc78dc7ba47460572a4286fffe989e9921abfd50791e01e4044d274"),
        },
        forks: Forks {
            genesis: Fork {
                epoch: 0,
                fork_version: fixed_bytes!("10585557"),
            },
            altair: Fork {
                epoch: 0,
                fork_version: fixed_bytes!("20585557"),
            },
            bellatrix: Fork {
                epoch: 0,
                fork_version: fixed_bytes!("30585557"),
            },
            capella: Fork {
                epoch: 0,
                fork_version: fixed_bytes!("40585557"),
            },
            deneb: Fork {
                epoch: 0,
                fork_version: fixed_bytes!("50585557"),
            },
            electra: Fork {
                epoch: 10,
                fork_version: fixed_bytes!("60585557"),
            },
        },
        execution_forks: ForkSchedule {
            prague_timestamp: 1738607700,
        },
        max_checkpoint_age: 1_209_600, // 14 days
        #[cfg(not(target_arch = "wasm32"))]
        data_dir: Some(data_dir(Network::PectraDevnet)),
        ..std::default::Default::default()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn data_dir(network: Network) -> PathBuf {
    home_dir()
        .unwrap()
        .join(format!(".helios/data/{}", network))
}
