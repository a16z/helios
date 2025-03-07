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
    pub verify_unsafe_signer: bool,
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
    pub verify_unsafe_signer: bool,
}

#[derive(Copy, Clone, Debug)]
pub enum Network {
    OpMainnet,
    Base,
    Worldchain,
    Zora,
    OpSepolia,
}

impl Display for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OpMainnet => f.write_str("op-mainnet"),
            Self::Base => f.write_str("base"),
            Self::Worldchain => f.write_str("worldchain"),
            Self::Zora => f.write_str("zora"),
            Self::OpSepolia => f.write_str("op-sepolia"),
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
            "op-sepolia" => Ok(Self::OpSepolia),
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
                },
                verify_unsafe_signer: false,
            },
            Network::Worldchain => NetworkConfig {
                consensus_rpc: Some("https://worldchain.operationsolarstorm.org".parse().unwrap()),
                chain: ChainConfig {
                    chain_id: 59144,
                    unsafe_signer: address!("6F54Ca6F6Ede96662024Ffd61BFd18f3f4e34DFf"),
                    system_config_contract: address!("F46a3e64CD3b30D36D7E3F4C60c6a5Fb2C4Fa24D"),
                    eth_network: EthNetwork::Mainnet,
                },
                verify_unsafe_signer: false,
            },
            Network::Zora => NetworkConfig {
                consensus_rpc: Some("https://zora.operationsolarstorm.org".parse().unwrap()),
                chain: ChainConfig {
                    chain_id: 7777777,
                    unsafe_signer: address!("5050F69a9786F081C3d9F8fE2573e1e7F6A3c236"),
                    system_config_contract: address!("F80Fb7FD87D3D14e265A0DFd1121C465891F9A23"),
                    eth_network: EthNetwork::Mainnet,
                },
                verify_unsafe_signer: false,
            },
            Network::OpSepolia => NetworkConfig {
                consensus_rpc: Some("https://op-sepolia.operationsolarstorm.org".parse().unwrap()),
                chain: ChainConfig {
                    chain_id: 11155420,
                    unsafe_signer: address!("6887246668a3b87F54DeB3b94Ba47a6f63F32985"),
                    system_config_contract: address!("4200000000000000000000000000000000000070"),
                    eth_network: EthNetwork::Sepolia,
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
