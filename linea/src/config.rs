use std::{
    collections::HashMap,
    default::Default,
    fmt::Display,
    net::{IpAddr, Ipv4Addr},
    path::PathBuf,
    process::exit,
    str::FromStr,
};

use figment::{
    providers::{Format, Serialized, Toml},
    value::Value,
    Figment,
};

use serde::{Deserialize, Serialize};

use eyre::Result;
use strum::EnumIter;
use url::Url;

use alloy::primitives::{address, Address};

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct ChainConfig {
    pub chain_id: u64,
    pub unsafe_signer: Address,
}

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, EnumIter, Hash, Eq, PartialEq, PartialOrd, Ord,
)]
pub enum Network {
    Mainnet,
    Sepolia,
}

impl FromStr for Network {
    type Err = eyre::Report;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "mainnet" => Ok(Self::Mainnet),
            "sepolia" => Ok(Self::Sepolia),
            _ => Err(eyre::eyre!("network not recognized")),
        }
    }
}

impl Display for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            Self::Mainnet => "mainnet",
            Self::Sepolia => "sepolia",
        };

        f.write_str(str)
    }
}

impl Network {
    pub fn to_base_config(&self) -> BaseConfig {
        match self {
            Self::Mainnet => mainnet(),
            Self::Sepolia => sepolia(),
        }
    }

    pub fn from_chain_id(id: u64) -> Result<Self> {
        match id {
            59144 => Ok(Network::Mainnet),
            59141 => Ok(Network::Sepolia),
            _ => Err(eyre::eyre!("chain id not known")),
        }
    }
}

pub fn mainnet() -> BaseConfig {
    BaseConfig {
        rpc_port: 8545,
        rpc_bind_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
        chain: ChainConfig {
            chain_id: 59144,
            unsafe_signer: address!("8f81e2e3f8b46467523463835f965ffe476e1c9e"),
        },
    }
}

pub fn sepolia() -> BaseConfig {
    BaseConfig {
        rpc_port: 8545,
        rpc_bind_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
        chain: ChainConfig {
            chain_id: 59141,
            unsafe_signer: address!("a27342f1b74c0cfb2cda74bac1628d0c1a9752f2"),
        },
    }
}

/// Cli Config
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct CliConfig {
    pub execution_rpc: Option<Url>,
    pub rpc_bind_ip: Option<IpAddr>,
    pub rpc_port: Option<u16>,
}

impl CliConfig {
    pub fn as_provider(&self, network: &str) -> Serialized<HashMap<&str, Value>> {
        let mut user_dict = HashMap::new();

        if let Some(rpc) = &self.execution_rpc {
            user_dict.insert("execution_rpc", Value::from(rpc.to_string()));
        }

        if let Some(ip) = self.rpc_bind_ip {
            user_dict.insert("rpc_bind_ip", Value::from(ip.to_string()));
        }

        if let Some(port) = self.rpc_port {
            user_dict.insert("rpc_port", Value::from(port));
        }

        Serialized::from(user_dict, network)
    }
}

/// The base configuration for a network.
#[derive(Serialize)]
pub struct BaseConfig {
    pub rpc_bind_ip: IpAddr,
    pub rpc_port: u16,
    pub chain: ChainConfig,
}

impl Default for BaseConfig {
    fn default() -> Self {
        BaseConfig {
            rpc_bind_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
            rpc_port: 0,
            chain: Default::default(),
        }
    }
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct Config {
    pub execution_rpc: String,
    pub rpc_bind_ip: Option<IpAddr>,
    pub rpc_port: Option<u16>,
    pub chain: ChainConfig,
}

impl Config {
    pub fn from_file(config_path: &PathBuf, network: &str, cli_config: &CliConfig) -> Self {
        let base_config = Network::from_str(network)
            .map(|n| n.to_base_config())
            .unwrap_or(BaseConfig::default());

        let base_provider = Serialized::from(base_config, network);
        let toml_provider = Toml::file(config_path).nested();
        let cli_provider = cli_config.as_provider(network);

        let config_res = Figment::new()
            .merge(base_provider)
            .merge(toml_provider)
            .merge(cli_provider)
            .select(network)
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
                    _ => println!("cannot parse configuration: {err}"),
                }
                exit(1);
            }
        }
    }

    pub fn to_base_config(&self) -> BaseConfig {
        BaseConfig {
            rpc_bind_ip: self.rpc_bind_ip.unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST)),
            rpc_port: self.rpc_port.unwrap_or(8545),
            chain: self.chain.clone(),
        }
    }
}

impl From<BaseConfig> for Config {
    fn from(base: BaseConfig) -> Self {
        Config {
            rpc_bind_ip: Some(base.rpc_bind_ip),
            rpc_port: Some(base.rpc_port),
            execution_rpc: String::new(),
            chain: base.chain,
        }
    }
}
