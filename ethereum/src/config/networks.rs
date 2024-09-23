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

use helios_consensus_core::types::{Fork, Forks};

use crate::config::base::BaseConfig;
use crate::config::types::ChainConfig;

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, EnumIter, Hash, Eq, PartialEq, PartialOrd, Ord,
)]
pub enum Network {
    MAINNET,
    GOERLI,
    SEPOLIA,
    HOLESKY,
}

impl FromStr for Network {
    type Err = eyre::Report;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "mainnet" => Ok(Self::MAINNET),
            "goerli" => Ok(Self::GOERLI),
            "sepolia" => Ok(Self::SEPOLIA),
            "holesky" => Ok(Self::HOLESKY),
            _ => Err(eyre::eyre!("network not recognized")),
        }
    }
}

impl Display for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            Self::MAINNET => "mainnet",
            Self::GOERLI => "goerli",
            Self::SEPOLIA => "sepolia",
            Self::HOLESKY => "holesky",
        };

        f.write_str(str)
    }
}

impl Network {
    pub fn to_base_config(&self) -> BaseConfig {
        match self {
            Self::MAINNET => mainnet(),
            Self::GOERLI => goerli(),
            Self::SEPOLIA => sepolia(),
            Self::HOLESKY => holesky(),
        }
    }

    pub fn from_chain_id(id: u64) -> Result<Self> {
        match id {
            1 => Ok(Network::MAINNET),
            5 => Ok(Network::GOERLI),
            11155111 => Ok(Network::SEPOLIA),
            17000 => Ok(Network::HOLESKY),
            _ => Err(eyre::eyre!("chain id not known")),
        }
    }
}

pub fn mainnet() -> BaseConfig {
    BaseConfig {
        default_checkpoint: b256!(
            "c7fc7b2f4b548bfc9305fa80bc1865ddc6eea4557f0a80507af5dc34db7bd9ce"
        ),
        rpc_port: 8545,
        consensus_rpc: Some("https://www.lightclientdata.org".to_string()),
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
        },
        max_checkpoint_age: 1_209_600, // 14 days
        #[cfg(not(target_arch = "wasm32"))]
        data_dir: Some(data_dir(Network::MAINNET)),
        ..std::default::Default::default()
    }
}

pub fn goerli() -> BaseConfig {
    BaseConfig {
        default_checkpoint: b256!(
            "f6e9d5fdd7c406834e888961beab07b2443b64703c36acc1274ae1ce8bb48839"
        ),
        rpc_port: 8545,
        consensus_rpc: None,
        chain: ChainConfig {
            chain_id: 5,
            genesis_time: 1616508000,
            genesis_root: b256!("043db0d9a83813551ee2f33450d23797757d430911a9320530ad8a0eabc43efb"),
        },
        forks: Forks {
            genesis: Fork {
                epoch: 0,
                fork_version: fixed_bytes!("00001020"),
            },
            altair: Fork {
                epoch: 36660,
                fork_version: fixed_bytes!("01001020"),
            },
            bellatrix: Fork {
                epoch: 112260,
                fork_version: fixed_bytes!("02001020"),
            },
            capella: Fork {
                epoch: 162304,
                fork_version: fixed_bytes!("03001020"),
            },
            deneb: Fork {
                epoch: 231680,
                fork_version: fixed_bytes!("04001020"),
            },
        },
        max_checkpoint_age: 1_209_600, // 14 days
        #[cfg(not(target_arch = "wasm32"))]
        data_dir: Some(data_dir(Network::GOERLI)),
        ..std::default::Default::default()
    }
}

pub fn sepolia() -> BaseConfig {
    BaseConfig {
        default_checkpoint: b256!(
            "4135bf01bddcfadac11143ba911f1c7f0772fdd6b87742b0bc229887bbf62b48"
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
        },
        max_checkpoint_age: 1_209_600, // 14 days
        #[cfg(not(target_arch = "wasm32"))]
        data_dir: Some(data_dir(Network::SEPOLIA)),
        ..std::default::Default::default()
    }
}

pub fn holesky() -> BaseConfig {
    BaseConfig {
        default_checkpoint: b256!(
            "d8fad84478f4947c3d09cfefde36d09bb9e71217f650610a3eb730eba54cdf1f"
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
        },
        max_checkpoint_age: 1_209_600, // 14 days
        #[cfg(not(target_arch = "wasm32"))]
        data_dir: Some(data_dir(Network::HOLESKY)),
        ..std::default::Default::default()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn data_dir(network: Network) -> PathBuf {
    home_dir()
        .unwrap()
        .join(format!(".helios/data/{}", network))
}
