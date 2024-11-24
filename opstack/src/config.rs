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
use helios_ethereum::config::networks::Network as EthNetwork;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    pub consensus_rpc: Url,
    pub execution_rpc: Url,
    pub rpc_socket: Option<SocketAddr>,
    pub chain: ChainConfig,
    pub load_external_fallback: Option<bool>,
    pub checkpoint: Option<B256>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ChainConfig {
    pub chain_id: u64,
    pub unsafe_signer: Address,
    pub system_config_contract: Address,
    pub eth_network: EthNetwork,
}

#[derive(Serialize, Deserialize)]
pub struct NetworkConfig {
    pub consensus_rpc: Option<Url>,
    pub chain: ChainConfig,
}

#[derive(Copy, Clone, Debug)]
pub enum Network {
    OpMainnet,
    Base,
    Worldchain,
    Zora,
}

impl Display for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OpMainnet => f.write_str("op-mainnet"),
            Self::Base => f.write_str("base"),
            Self::Worldchain => f.write_str("worldchain"),
            Self::Zora => f.write_str("zora"),
        }
    }
}

impl FromStr for Network {
    type Err = eyre::Report;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "op-mainnet" => Ok(Self::OpMainnet),
            "base" => Ok(Self::Base),
            "worldchain" => Ok(Self::Worldchain),
            "zora" => Ok(Self::Zora),
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
                    eth_network: EthNetwork::MAINNET,
                },
            },
            Network::Base => NetworkConfig {
                consensus_rpc: Some("https://base.operationsolarstorm.org".parse().unwrap()),
                chain: ChainConfig {
                    chain_id: 8453,
                    unsafe_signer: address!("Af6E19BE0F9cE7f8afd49a1824851023A8249e8a"),
                    system_config_contract: address!("73a79Fab69143498Ed3712e519A88a918e1f4072"),
                    eth_network: EthNetwork::MAINNET,
                },
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
                    eth_network: EthNetwork::MAINNET,
                },
            },
            Network::Zora => NetworkConfig {
                consensus_rpc: Some("https://zora.operationsolarstorm.org".parse().unwrap()),
                chain: ChainConfig {
                    chain_id: 7777777,
                    unsafe_signer: address!("3Dc8Dfd0709C835cAd15a6A27e089FF4cF4C9228"),
                    system_config_contract: address!("A3cAB0126d5F504B071b81a3e8A2BBBF17930d86"),
                    eth_network: EthNetwork::MAINNET,
                },
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
