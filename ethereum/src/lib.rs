use consensus::ConsensusClient;
use helios_consensus_core::consensus_spec::MainnetConsensusSpec;
use helios_core::client::Client;
use rpc::http_rpc::HttpRpc;
use spec::Ethereum;

pub mod builder;
pub mod config;
pub mod consensus;
pub mod database;
pub mod rpc;
pub mod spec;

mod constants;

pub use builder::EthereumClientBuilder;
pub type EthereumClient<E, DB> =
    Client<Ethereum, ConsensusClient<MainnetConsensusSpec, HttpRpc, DB>, E>;
