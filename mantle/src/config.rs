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
    MantleMainnet,
    MantleSepolia,
}

impl Display for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MantleMainnet => f.write_str("mantle"),
            Self::MantleSepolia => f.write_str("mantle-sepolia"),
        }
    }
}

impl FromStr for Network {
    type Err = eyre::Report;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "mantle" => Ok(Self::MantleMainnet),
            "mantle-sepolia" => Ok(Self::MantleSepolia),
            _ => Err(eyre::eyre!("network not recognized")),
        }
    }
}

impl From<Network> for NetworkConfig {
    fn from(value: Network) -> Self {
        match value {
            Network::MantleMainnet => NetworkConfig {
                // TODO: Replace with Mantle's own consensus server URL once available.
                // Mantle runs its own sequencer infrastructure; a Helios-compatible consensus
                // server needs to be deployed for Mantle (similar to operationsolarstorm.org
                // for standard OP Stack chains).
                consensus_rpc: None,
                chain: ChainConfig {
                    chain_id: 5000,
                    // Mantle L2 unsafe block signer (p2pSequencerAddress from deploy config).
                    // Source: mantle-v2/packages/contracts-bedrock/deploy-config/mantle-mainnet.json
                    // Stored in SystemConfig at slot 0x65a7ed542fb37fe237fdfbdd70b31598523fe5b32879e307bae27a0bd9581c08.
                    // Use `verify_unsafe_signer: true` to fetch the latest value trustlessly from L1.
                    unsafe_signer: address!("aac979cbee00c75c35de9a2635d8b75940f466dc"),
                    // Mantle L1 SystemConfigProxy on Ethereum mainnet
                    // https://docs.mantle.xyz/network/system-information/on-chain-system/key-l1-contract-address
                    system_config_contract: address!(
                        "427Ea0710FA5252057F0D88274f7aeb308386cAf"
                    ),
                    eth_network: EthNetwork::Mainnet,
                    forks: MantleForkSchedule::mainnet(),
                },
                verify_unsafe_signer: false,
            },
            Network::MantleSepolia => NetworkConfig {
                // TODO: Replace with Mantle Sepolia consensus server URL once available.
                consensus_rpc: None,
                chain: ChainConfig {
                    chain_id: 5003,
                    // Mantle Sepolia unsafe block signer (p2pSequencerAddress from deploy config).
                    // Source: mantle-v2/packages/contracts-bedrock/deploy-config/mantle-sepolia.json
                    unsafe_signer: address!("ff17e3dd9c0e0378ab219111f5325c83ebd01d31"),
                    // Mantle Sepolia L1 SystemConfigProxy on Ethereum Sepolia
                    // https://docs.mantle.xyz/network/system-information/on-chain-system/key-l1-contract-address
                    system_config_contract: address!(
                        "04B34526C91424e955D13C7226Bc4385e57E6706"
                    ),
                    eth_network: EthNetwork::Sepolia,
                    forks: MantleForkSchedule::sepolia(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_from_str_mainnet() {
        let network = Network::from_str("mantle").unwrap();
        assert!(matches!(network, Network::MantleMainnet));
    }

    #[test]
    fn test_network_from_str_sepolia() {
        let network = Network::from_str("mantle-sepolia").unwrap();
        assert!(matches!(network, Network::MantleSepolia));
    }

    #[test]
    fn test_network_from_str_invalid() {
        let result = Network::from_str("unknown");
        assert!(result.is_err());
        let result2 = Network::from_str("ethereum");
        assert!(result2.is_err());
        let result3 = Network::from_str("");
        assert!(result3.is_err());
    }

    #[test]
    fn test_network_display_mainnet() {
        assert_eq!(Network::MantleMainnet.to_string(), "mantle");
    }

    #[test]
    fn test_network_display_sepolia() {
        assert_eq!(Network::MantleSepolia.to_string(), "mantle-sepolia");
    }

    #[test]
    fn test_network_display_roundtrip() {
        let mainnet = Network::MantleMainnet;
        let parsed = Network::from_str(&mainnet.to_string()).unwrap();
        assert!(matches!(parsed, Network::MantleMainnet));

        let sepolia = Network::MantleSepolia;
        let parsed = Network::from_str(&sepolia.to_string()).unwrap();
        assert!(matches!(parsed, Network::MantleSepolia));
    }

    #[test]
    fn test_network_config_mainnet() {
        let config = NetworkConfig::from(Network::MantleMainnet);
        assert_eq!(config.chain.chain_id, 5000);
        assert_eq!(
            config.chain.unsafe_signer,
            address!("aac979cbee00c75c35de9a2635d8b75940f466dc")
        );
        assert_eq!(
            config.chain.system_config_contract,
            address!("427Ea0710FA5252057F0D88274f7aeb308386cAf")
        );
        assert!(matches!(config.chain.eth_network, EthNetwork::Mainnet));
        assert!(!config.verify_unsafe_signer);
        assert!(config.consensus_rpc.is_none());
    }

    #[test]
    fn test_network_config_sepolia() {
        let config = NetworkConfig::from(Network::MantleSepolia);
        assert_eq!(config.chain.chain_id, 5003);
        assert_eq!(
            config.chain.unsafe_signer,
            address!("ff17e3dd9c0e0378ab219111f5325c83ebd01d31")
        );
        assert_eq!(
            config.chain.system_config_contract,
            address!("04B34526C91424e955D13C7226Bc4385e57E6706")
        );
        assert!(matches!(config.chain.eth_network, EthNetwork::Sepolia));
        assert!(!config.verify_unsafe_signer);
        assert!(config.consensus_rpc.is_none());
    }

    #[test]
    fn test_mainnet_fork_schedule() {
        let forks = MantleForkSchedule::mainnet();
        const SKADI: u64 = 1_756_278_000;
        const LIMB: u64 = 1_768_374_000;

        assert_eq!(forks.bedrock_timestamp, 0);
        assert_eq!(forks.regolith_timestamp, 0);
        // EVM-level forks are set to Skadi/Limb
        assert_eq!(forks.shanghai_timestamp, SKADI);
        assert_eq!(forks.cancun_timestamp, SKADI);
        assert_eq!(forks.prague_timestamp, SKADI);
        assert_eq!(forks.osaka_timestamp, LIMB);
    }

    #[test]
    fn test_sepolia_fork_schedule() {
        let forks = MantleForkSchedule::sepolia();
        const SKADI: u64 = 1_752_649_200;
        const LIMB: u64 = 1_764_745_200;

        assert_eq!(forks.bedrock_timestamp, 0);
        assert_eq!(forks.regolith_timestamp, 0);
        // EVM-level forks are set to Skadi/Limb
        assert_eq!(forks.shanghai_timestamp, SKADI);
        assert_eq!(forks.cancun_timestamp, SKADI);
        assert_eq!(forks.prague_timestamp, SKADI);
        assert_eq!(forks.osaka_timestamp, LIMB);
    }

    #[test]
    fn test_op_forks_not_activated() {
        let forks = MantleForkSchedule::mainnet();
        // OP-level forks should remain at u64::MAX (MantleArsiaTime is nil)
        assert_eq!(forks.canyon_timestamp, u64::MAX);
        assert_eq!(forks.delta_timestamp, u64::MAX);
        assert_eq!(forks.ecotone_timestamp, u64::MAX);
        assert_eq!(forks.fjord_timestamp, u64::MAX);
        assert_eq!(forks.granite_timestamp, u64::MAX);
        assert_eq!(forks.holocene_timestamp, u64::MAX);
        assert_eq!(forks.isthmus_timestamp, u64::MAX);
        assert_eq!(forks.jovian_timestamp, u64::MAX);
    }

    #[test]
    fn test_sepolia_activates_before_mainnet() {
        let mainnet = MantleForkSchedule::mainnet();
        let sepolia = MantleForkSchedule::sepolia();
        // Sepolia EVM forks activate earlier than mainnet (testnet first)
        assert!(sepolia.prague_timestamp < mainnet.prague_timestamp);
        assert!(sepolia.osaka_timestamp < mainnet.osaka_timestamp);
    }

    #[test]
    fn test_blob_base_fee_fraction_at_skadi() {
        let forks = MantleForkSchedule::mainnet();
        // Before SkadiTime: Cancun fraction
        assert_eq!(forks.get_blob_base_fee_update_fraction(1_000_000), 3338477);
        // At SkadiTime: Prague fraction (EIP-7892)
        assert_eq!(forks.get_blob_base_fee_update_fraction(1_756_278_000), 5007716);
    }

    #[test]
    fn test_mainnet_and_sepolia_different_chain_ids() {
        let mainnet_config = NetworkConfig::from(Network::MantleMainnet);
        let sepolia_config = NetworkConfig::from(Network::MantleSepolia);
        assert_ne!(mainnet_config.chain.chain_id, sepolia_config.chain.chain_id);
    }
}

/// Mantle-specific fork schedule.
///
/// Mantle is based on OP Stack (Bedrock) but activates EVM features on its own
/// upgrade cadence rather than the standard Superchain schedule:
///
///   MantleSkadiTime → Shanghai / Cancun / Prague EVM level
///   MantleLimbTime  → Osaka EVM level
///
/// In Go (op-geth/core/genesis.go), the two fork dimensions are set separately:
///
///   OP-level forks (Canyon..Jovian) → MantleArsiaTime  (nil on mainnet/sepolia)
///   EVM-level forks (Shanghai/Cancun/Prague) → MantleSkadiTime
///   EVM-level fork  (Osaka) → MantleLimbTime
///
/// We mirror this by setting the EVM-level fork timestamps (prague, osaka) and
/// leaving OP-level forks at u64::MAX (the nil equivalent). The Mantle EVM
/// selector checks both dimensions so the correct OpSpecId is resolved.
///
/// Mantle v2 Tectonic upgrade (Bedrock-based) launched on March 15, 2024.
///
/// Source: op-geth/params/mantle.go, op-geth/core/genesis.go
pub struct MantleForkSchedule;

impl MantleForkSchedule {
    /// Mantle Mainnet fork schedule.
    ///
    /// Source: op-geth/params/mantle.go (MantleMainnetUpgradeConfig)
    ///
    /// MantleSkadiTime = 1_756_278_000 — activates Prague-level EVM
    /// MantleLimbTime  = 1_768_374_000 — activates Osaka-level EVM
    pub fn mainnet() -> ForkSchedule {
        const MANTLE_SKADI_TIME: u64 = 1_756_278_000;
        const MANTLE_LIMB_TIME: u64 = 1_768_374_000;

        ForkSchedule {
            bedrock_timestamp: 0,
            regolith_timestamp: 0,
            // EVM-level forks matching Go's genesis.go:
            //   cfg.ShanghaiTime = cfg.MantleSkadiTime
            //   cfg.CancunTime   = cfg.MantleSkadiTime
            //   cfg.PragueTime   = cfg.MantleSkadiTime
            //   cfg.OsakaTime    = cfg.MantleLimbTime
            shanghai_timestamp: MANTLE_SKADI_TIME,
            cancun_timestamp: MANTLE_SKADI_TIME,
            prague_timestamp: MANTLE_SKADI_TIME,
            osaka_timestamp: MANTLE_LIMB_TIME,
            // OP-level forks: MantleArsiaTime is nil on mainnet (not yet scheduled).
            // All OP forks stay at u64::MAX (default).
            ..Default::default()
        }
    }

    /// Mantle Sepolia (testnet) fork schedule.
    ///
    /// Source: op-geth/params/mantle.go (MantleSepoliaUpgradeConfig)
    ///
    /// MantleSkadiTime = 1_752_649_200 — activates Prague-level EVM
    /// MantleLimbTime  = 1_764_745_200 — activates Osaka-level EVM
    pub fn sepolia() -> ForkSchedule {
        const MANTLE_SKADI_TIME: u64 = 1_752_649_200;
        const MANTLE_LIMB_TIME: u64 = 1_764_745_200;

        ForkSchedule {
            bedrock_timestamp: 0,
            regolith_timestamp: 0,
            // EVM-level forks matching Go's genesis.go
            shanghai_timestamp: MANTLE_SKADI_TIME,
            cancun_timestamp: MANTLE_SKADI_TIME,
            prague_timestamp: MANTLE_SKADI_TIME,
            osaka_timestamp: MANTLE_LIMB_TIME,
            // OP-level forks: MantleArsiaTime is nil on sepolia (not yet scheduled).
            // All OP forks stay at u64::MAX (default).
            ..Default::default()
        }
    }
}
