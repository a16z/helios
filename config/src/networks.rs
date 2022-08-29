use common::utils::hex_str_to_bytes;

use crate::{Config, Fork, Forks, General};

pub fn goerli() -> Config {
    Config {
        general: General {
            chain_id: 5,
            genesis_root: hex_str_to_bytes(
                "0x043db0d9a83813551ee2f33450d23797757d430911a9320530ad8a0eabc43efb",
            )
            .unwrap(),
            checkpoint: hex_str_to_bytes(
                "0x172128eadf1da46467f4d6a822206698e2d3f957af117dd650954780d680dc99",
            )
            .unwrap(),
            consensus_rpc: "http://testing.prater.beacon-api.nimbus.team".to_string(),
            execution_rpc:
                "https://eth-goerli.g.alchemy.com:443/v2/o_8Qa9kgwDPf9G8sroyQ-uQtyhyWa3ao"
                    .to_string(),
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
