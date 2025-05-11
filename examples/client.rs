use std::path::PathBuf;

use alloy::primitives::b256;
use eyre::Result;
use helios::ethereum::{
    config::networks::Network, database::FileDB, EthereumClient, EthereumClientBuilder,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Create a new Helios Client Builder
    let mut builder = EthereumClientBuilder::new();

    // Set the network to mainnet
    builder = builder.network(Network::Mainnet);

    // Set the consensus rpc url
    builder = builder.consensus_rpc("https://www.lightclientdata.org");

    // Set the execution rpc url
    builder = builder.execution_rpc("https://eth-mainnet.g.alchemy.com/v2/XXXXX");

    // Set the checkpoint to the last known checkpoint
    builder = builder.checkpoint(b256!(
        "85e6151a246e8fdba36db27a0c7678a575346272fe978c9281e13a8b26cdfa68"
    ));

    // Set the rpc port
    builder = builder.rpc_port(8545);

    // Set the data dir
    builder = builder.data_dir(PathBuf::from("/tmp/helios"));

    // Set the fallback service
    builder = builder.fallback("https://sync-mainnet.beaconcha.in");

    // Enable lazy checkpoints
    builder = builder.load_external_fallback();

    // Build the client
    let _client: EthereumClient<FileDB> = builder.build().unwrap();
    println!("Constructed client!");

    Ok(())
}
