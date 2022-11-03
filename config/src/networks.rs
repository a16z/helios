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
    #[serde(
        deserialize_with = "bytes_deserialize",
        serialize_with = "bytes_serialize"
    )]
    pub checkpoint: Vec<u8>,
    pub chain: ChainConfig,
    pub forks: Forks,
}

pub fn mainnet() -> BaseConfig {
    BaseConfig {
        checkpoint: hex_str_to_bytes(
            "0x6d41048663adafa064bae3b3768a8448fa1f1b003118fa5887d06da266530cff",
        )
        .unwrap(),
        rpc_port: 8545,
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
    }
}

pub fn goerli() -> BaseConfig {
    BaseConfig {
        checkpoint: hex_str_to_bytes(
            "0x1e591af1e90f2db918b2a132991c7c2ee9a4ab26da496bd6e71e4f0bd65ea870",
        )
        .unwrap(),
        rpc_port: 8545,
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
    }
}
