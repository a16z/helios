use std::{
    collections::HashMap, fmt::Display, net::SocketAddr, path::PathBuf, process::exit, str::FromStr,
};

use alloy::primitives::{address, Address, B256};
use eyre::Result;
use figment::{
    providers::{Format, Serialized, Toml},
    value::Value,
    Figment,
};
use serde::{Deserialize, Serialize};
use url::Url;

use helios_common::fork_schedule::ForkSchedule;
use helios_ethereum::config::networks::Network as EthNetwork;

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    pub consensus_rpc: Url,
    pub execution_rpc: Option<Url>,
    pub verifiable_api: Option<Url>,
    pub rpc_socket: Option<SocketAddr>,
    pub allowed_origins: Option<Vec<String>>,
    pub chain: ChainConfig,
    pub load_external_fallback: Option<bool>,
    pub checkpoint: Option<B256>,
    pub verify_unsafe_signer: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ChainConfig {
    pub chain_id: u64,
    pub unsafe_signer: Address,
    pub system_config_contract: Address,
    pub eth_network: EthNetwork,
    pub forks: ForkSchedule,
}

#[derive(Serialize, Deserialize)]
pub struct NetworkConfig {
    pub consensus_rpc: Option<Url>,
    pub chain: ChainConfig,
    pub verify_unsafe_signer: bool,
}

#[derive(Copy, Clone, Debug)]
pub enum Network {
    OpMainnet,
    Base,
    BaseSepolia,
    Worldchain,
    Zora,
    Unichain,
}

impl Display for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OpMainnet => f.write_str("op-mainnet"),
            Self::Base => f.write_str("base"),
            Self::BaseSepolia => f.write_str("base-sepolia"),
            Self::Worldchain => f.write_str("worldchain"),
            Self::Zora => f.write_str("zora"),
            Self::Unichain => f.write_str("unichain"),
        }
    }
}

impl FromStr for Network {
    type Err = eyre::Report;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "op-mainnet" => Ok(Self::OpMainnet),
            "base" => Ok(Self::Base),
            "base-sepolia" => Ok(Self::BaseSepolia),
            "worldchain" => Ok(Self::Worldchain),
            "zora" => Ok(Self::Zora),
            "unichain" => Ok(Self::Unichain),
            _ => Err(eyre::eyre!("network not recognized")),
        }
    }
}

impl From<Network> for NetworkConfig {
    fn from(value: Network) -> Self {
        match value {
            Network::OpMainnet => NetworkConfig {
                consensus_rpc: Some(
                    "https://op-mainnet.operationsolarstorm.org"
                        .parse()
                        .unwrap(),
                ),
                chain: ChainConfig {
                    chain_id: 10,
                    unsafe_signer: address!("AAAA45d9549EDA09E70937013520214382Ffc4A2"),
                    system_config_contract: address!("229047fed2591dbec1eF1118d64F7aF3dB9EB290"),
                    eth_network: EthNetwork::Mainnet,
                    forks: SuperchainForkSchedule::mainnet(),
                },
                verify_unsafe_signer: false,
            },
            Network::Base => NetworkConfig {
                consensus_rpc: Some("https://base.operationsolarstorm.org".parse().unwrap()),
                chain: ChainConfig {
                    chain_id: 8453,
                    unsafe_signer: address!("Af6E19BE0F9cE7f8afd49a1824851023A8249e8a"),
                    system_config_contract: address!("73a79Fab69143498Ed3712e519A88a918e1f4072"),
                    eth_network: EthNetwork::Mainnet,
                    forks: SuperchainForkSchedule::mainnet(),
                },
                verify_unsafe_signer: false,
            },
            Network::BaseSepolia => NetworkConfig {
                consensus_rpc: Some(
                    "https://base-sepolia.operationsolarstorm.org"
                        .parse()
                        .unwrap(),
                ),
                chain: ChainConfig {
                    chain_id: 84532,
                    unsafe_signer: address!("0xb830b99c95Ea32300039624Cb567d324D4b1D83C"),
                    system_config_contract: address!("0xf272670eb55e895584501d564AfEB048bEd26194"),
                    eth_network: EthNetwork::Sepolia,
                    forks: SuperchainForkSchedule::sepolia(),
                },
                verify_unsafe_signer: false,
            },
            Network::Worldchain => NetworkConfig {
                consensus_rpc: Some(
                    "https://worldchain.operationsolarstorm.org"
                        .parse()
                        .unwrap(),
                ),
                chain: ChainConfig {
                    chain_id: 480,
                    unsafe_signer: address!("2270d6eC8E760daA317DD978cFB98C8f144B1f3A"),
                    system_config_contract: address!("6ab0777fD0e609CE58F939a7F70Fe41F5Aa6300A"),
                    eth_network: EthNetwork::Mainnet,
                    forks: SuperchainForkSchedule::mainnet(),
                },
                verify_unsafe_signer: false,
            },
            Network::Zora => NetworkConfig {
                consensus_rpc: Some("https://zora.operationsolarstorm.org".parse().unwrap()),
                chain: ChainConfig {
                    chain_id: 7777777,
                    unsafe_signer: address!("3Dc8Dfd0709C835cAd15a6A27e089FF4cF4C9228"),
                    system_config_contract: address!("A3cAB0126d5F504B071b81a3e8A2BBBF17930d86"),
                    eth_network: EthNetwork::Mainnet,
                    forks: SuperchainForkSchedule::mainnet(),
                },
                verify_unsafe_signer: false,
            },
            Network::Unichain => NetworkConfig {
                consensus_rpc: Some("https://unichain.operationsolarstorm.org".parse().unwrap()),
                chain: ChainConfig {
                    chain_id: 130,
                    unsafe_signer: address!("0x833C6f278474A78658af91aE8edC926FE33a230e"),
                    system_config_contract: address!("c407398d063f942feBbcC6F80a156b47F3f1BDA6"),
                    eth_network: EthNetwork::Mainnet,
                    forks: SuperchainForkSchedule::mainnet(),
                },
                verify_unsafe_signer: false,
            },
        }
    }
}

impl Config {
    pub fn from_file(
        config_path: &PathBuf,
        network: &str,
        cli_provider: Serialized<HashMap<&str, Value>>,
    ) -> Self {
        let network = Network::from_str(network).unwrap();
        let network_config = NetworkConfig::from(network);

        let base_provider = Serialized::from(network_config, network.to_string());
        let toml_provider = Toml::file(config_path).nested();

        let config_res = Figment::new()
            .merge(base_provider)
            .merge(toml_provider)
            .merge(cli_provider)
            .select(network.to_string())
            .extract();

        match config_res {
            Ok(config) => config,
            Err(err) => {
                match err.kind {
                    figment::error::Kind::MissingField(field) => {
                        let field = field.replace('_', "-");
                        println!("\x1b[91merror\x1b[0m: missing configuration field: {field}");
                        println!("\n\ttry supplying the proper command line argument: --{field}");
                        println!("\talternatively, you can add the field to your helios.toml file");
                        println!("\nfor more information, check the github README");
                    }
                    figment::error::Kind::InvalidType(_, _) => {
                        let field = err.path.join(".").replace("_", "-");
                        println!("\x1b[91merror\x1b[0m: invalid configuration field: {field}");
                        println!("\n\ttry supplying the proper command line argument: --{field}");
                        println!("\talternatively, you can add the field to your helios.toml file");
                        println!("\nfor more information, check the github README");
                    }
                    _ => println!("cannot parse configuration: {err}"),
                }
                exit(1);
            }
        }
    }
}

pub struct SuperchainForkSchedule;

impl SuperchainForkSchedule {
    /// Mainnet network upgrade schedule
    /// https://docs.optimism.io/op-stack/protocol/network-upgrades
    pub fn mainnet() -> ForkSchedule {
        ForkSchedule {
            bedrock_timestamp: 0,
            regolith_timestamp: 0,
            canyon_timestamp: 1704992401,
            delta_timestamp: 1708560000,
            ecotone_timestamp: 1710374401,
            fjord_timestamp: 1720627201,
            granite_timestamp: 1726070401,
            holocene_timestamp: 1736445601,
            isthmus_timestamp: 1744905600,
            jovian_timestamp: 1764691201,

            ..Default::default()
        }
    }

    /// Sepolia network upgrade schedule
    /// https://docs.optimism.io/op-stack/protocol/network-upgrades
    pub fn sepolia() -> ForkSchedule {
        ForkSchedule {
            bedrock_timestamp: 0,
            regolith_timestamp: 0,
            canyon_timestamp: 1699981200,   // 2023-11-14 17:00:00 UTC
            delta_timestamp: 1703203200,    // 2023-12-22 00:00:00 UTC
            ecotone_timestamp: 1708534800,  // 2024-02-21 17:00:00 UTC
            fjord_timestamp: 1716998400,    // 2024-05-29 16:00:00 UTC
            granite_timestamp: 1723478400,  // 2024-08-12 16:00:00 UTC
            holocene_timestamp: 1732633200, // 2024-11-26 15:00:00 UTC
            isthmus_timestamp: 1744905600,  // 2025-04-17 16:00:00 UTC
            jovian_timestamp: 1763568001,   // 2025-11-19 16:00:01 UTC

            ..Default::default()
        }
    }
}
