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
use url::Url;

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
    Hoodi,
}

impl FromStr for Network {
    type Err = eyre::Report;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "mainnet" => Ok(Self::Mainnet),
            "sepolia" => Ok(Self::Sepolia),
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
            Self::Hoodi => hoodi(),
        }
    }

    pub fn from_chain_id(id: u64) -> Result<Self> {
        match id {
            1 => Ok(Network::Mainnet),
            11155111 => Ok(Network::Sepolia),
            560048 => Ok(Network::Hoodi),
            _ => Err(eyre::eyre!("chain id not known")),
        }
    }
}

pub fn mainnet() -> BaseConfig {
    BaseConfig {
        default_checkpoint: b256!(
            "9b41a80f58c52068a00e8535b8d6704769c7577a5fd506af5e0c018687991d55"
        ),
        rpc_port: 8545,
        consensus_rpc: Some(Url::parse("https://ethereum.operationsolarstorm.org").unwrap()),
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
            fulu: Fork {
                epoch: 411392,
                fork_version: fixed_bytes!("06000000"),
            },
        },
        execution_forks: EthereumForkSchedule::mainnet(),
        max_checkpoint_age: 1_209_600, // 14 days
        #[cfg(not(target_arch = "wasm32"))]
        data_dir: Some(data_dir(Network::Mainnet)),
        ..std::default::Default::default()
    }
}

pub fn sepolia() -> BaseConfig {
    BaseConfig {
        default_checkpoint: b256!(
            "4065c2509eaa15dbe60e1f80cff5205a532aa95aaa1d73c1c286f7f8535555d4"
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
            fulu: Fork {
                epoch: 272640,
                fork_version: fixed_bytes!("90000075"),
            },
        },
        execution_forks: EthereumForkSchedule::sepolia(),
        max_checkpoint_age: 1_209_600, // 14 days
        #[cfg(not(target_arch = "wasm32"))]
        data_dir: Some(data_dir(Network::Sepolia)),
        ..std::default::Default::default()
    }
}

pub fn hoodi() -> BaseConfig {
    BaseConfig {
        default_checkpoint: b256!(
            "3335028555f5fff431f82f978d2503ed59bc8da00a86217eea9befa9d486a049"
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
            fulu: Fork {
                epoch: 50688,
                fork_version: fixed_bytes!("70000910"),
            },
        },
        execution_forks: EthereumForkSchedule::hoodi(),
        max_checkpoint_age: 1_209_600, // 14 days
        #[cfg(not(target_arch = "wasm32"))]
        data_dir: Some(data_dir(Network::Hoodi)),
        ..std::default::Default::default()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn data_dir(network: Network) -> PathBuf {
    match home_dir() {
        Some(home) => home.join(format!(".helios/data/{network}")),
        None => std::env::temp_dir().join(format!("helios/data/{network}")),
    }
}

pub struct EthereumForkSchedule;

impl EthereumForkSchedule {
    fn mainnet() -> ForkSchedule {
        ForkSchedule {
            frontier_timestamp: 1438226773,
            homestead_timestamp: 1457938193,
            dao_timestamp: 1468977640,
            tangerine_timestamp: 1476753571,
            spurious_dragon_timestamp: 1479788144,
            byzantium_timestamp: 1508131331,
            constantinople_timestamp: 1551340324,
            petersburg_timestamp: 1551340324,
            istanbul_timestamp: 1575807909,
            muir_glacier_timestamp: 1577953849,
            berlin_timestamp: 1618481223,
            london_timestamp: 1628166822,
            arrow_glacier_timestamp: 1639036523,
            gray_glacier_timestamp: 1656586444,
            paris_timestamp: 1663224162,
            shanghai_timestamp: 1681338455,
            cancun_timestamp: 1710338135,
            prague_timestamp: 1746612311,
            osaka_timestamp: 1764798551,

            ..Default::default()
        }
    }

    fn sepolia() -> ForkSchedule {
        ForkSchedule {
            frontier_timestamp: 1633267481,
            homestead_timestamp: 1633267481,
            dao_timestamp: 1633267481,
            tangerine_timestamp: 1633267481,
            spurious_dragon_timestamp: 1633267481,
            byzantium_timestamp: 1633267481,
            constantinople_timestamp: 1633267481,
            petersburg_timestamp: 1633267481,
            istanbul_timestamp: 1633267481,
            muir_glacier_timestamp: 1633267481,
            berlin_timestamp: 1633267481,
            london_timestamp: 1633267481,
            arrow_glacier_timestamp: 1633267481,
            gray_glacier_timestamp: 1633267481,
            paris_timestamp: 1633267481,
            shanghai_timestamp: 1677557088,
            cancun_timestamp: 1706655072,
            prague_timestamp: 1741159776,
            osaka_timestamp: 1760427360,

            ..Default::default()
        }
    }

    fn hoodi() -> ForkSchedule {
        ForkSchedule {
            frontier_timestamp: 0,
            homestead_timestamp: 0,
            dao_timestamp: 0,
            tangerine_timestamp: 0,
            spurious_dragon_timestamp: 0,
            byzantium_timestamp: 0,
            constantinople_timestamp: 0,
            petersburg_timestamp: 0,
            istanbul_timestamp: 0,
            muir_glacier_timestamp: 0,
            berlin_timestamp: 0,
            london_timestamp: 0,
            arrow_glacier_timestamp: 0,
            gray_glacier_timestamp: 0,
            paris_timestamp: 0,
            shanghai_timestamp: 0,
            cancun_timestamp: 0,
            prague_timestamp: 1742999832,
            osaka_timestamp: 1761677592,

            ..Default::default()
        }
    }
}
