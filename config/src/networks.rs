use std::fmt::Display;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
use std::str::FromStr;

use common::utils::hex_str_to_bytes;
#[cfg(not(target_arch = "wasm32"))]
use dirs::home_dir;
use eyre::Result;
use serde::{Deserialize, Serialize};
use strum::EnumIter;

use crate::base::BaseConfig;
use crate::types::{ChainConfig, Fork, Forks};

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
            deneb: Fork {
                epoch: 269568,
                fork_version: hex_str_to_bytes("0x04000000").unwrap(),
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
            deneb: Fork {
                epoch: 231680,
                fork_version: hex_str_to_bytes("0x04001020").unwrap(),
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
            deneb: Fork {
                epoch: 132608,
                fork_version: hex_str_to_bytes("0x90000073").unwrap(),
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
        default_checkpoint: hex_str_to_bytes(
            "0xd8fad84478f4947c3d09cfefde36d09bb9e71217f650610a3eb730eba54cdf1f",
        )
        .unwrap(),
        rpc_port: 8545,
        consensus_rpc: None,
        chain: ChainConfig {
            chain_id: 17000,
            genesis_time: 1695902400,
            genesis_root: hex_str_to_bytes(
                "0x9143aa7c615a7f7115e2b6aac319c03529df8242ae705fba9df39b79c59fa8b1",
            )
            .unwrap(),
        },
        forks: Forks {
            genesis: Fork {
                epoch: 0,
                fork_version: hex_str_to_bytes("0x01017000").unwrap(),
            },
            altair: Fork {
                epoch: 0,
                fork_version: hex_str_to_bytes("0x02017000").unwrap(),
            },
            bellatrix: Fork {
                epoch: 0,
                fork_version: hex_str_to_bytes("0x03017000").unwrap(),
            },
            capella: Fork {
                epoch: 256,
                fork_version: hex_str_to_bytes("0x04017000").unwrap(),
            },
            deneb: Fork {
                epoch: 29696,
                fork_version: hex_str_to_bytes("0x05017000").unwrap(),
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
