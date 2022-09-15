use common::utils::hex_str_to_bytes;

use crate::{Config, Fork, Forks, General};

pub fn mainnet() -> Config {
    Config {
        general: General {
            chain_id: 1,
            genesis_time: 1606824023,
            genesis_root: hex_str_to_bytes(
                "0x4b363db94e286120d76eb905340fdd4e54bfe9f06bf33ff6cf5ad27f511bfe95",
            )
            .unwrap(),
            checkpoint: hex_str_to_bytes(
                "0x03e315e11b3f88cd63dfb62c74a313c4a65949ce9e37599e0ee66533ceceadfd",
            )
            .unwrap(),
            consensus_rpc: "http://testing.mainnet.beacon-api.nimbus.team".to_string(),
            execution_rpc: "https://eth-mainnet.g.alchemy.com/v2/Q0BqQPbTQfSMzrCNl4x80XS_PLLB1RNf"
                .to_string(),
            rpc_port: Some(8545),
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

pub fn goerli() -> Config {
    Config {
        general: General {
            chain_id: 5,
            genesis_time: 1616508000,
            genesis_root: hex_str_to_bytes(
                "0x043db0d9a83813551ee2f33450d23797757d430911a9320530ad8a0eabc43efb",
            )
            .unwrap(),
            checkpoint: hex_str_to_bytes(
                "0x1e591af1e90f2db918b2a132991c7c2ee9a4ab26da496bd6e71e4f0bd65ea870",
            )
            .unwrap(),
            consensus_rpc: "http://34.207.158.131:5052".to_string(),
            execution_rpc:
                "https://eth-goerli.g.alchemy.com:443/v2/o_8Qa9kgwDPf9G8sroyQ-uQtyhyWa3ao"
                    .to_string(),
            rpc_port: Some(8545),
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
