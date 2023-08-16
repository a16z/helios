use std::path::PathBuf;

use eyre::Result;

use helios::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Create a new Helios Client Builder
    let mut builder = ClientBuilder::new();

    // Set the network to mainnet
    builder = builder.network(networks::Network::MAINNET);

    // Set the consensus rpc url
    builder = builder.consensus_rpc("https://www.lightclientdata.org");

    // Set the execution rpc url
    builder = builder.execution_rpc("https://eth-mainnet.g.alchemy.com/v2/XXXXX");

    // Set the checkpoint to the last known checkpoint
    builder =
        builder.checkpoint("85e6151a246e8fdba36db27a0c7678a575346272fe978c9281e13a8b26cdfa68");

    // Set the rpc port
    builder = builder.rpc_port(8545);

    // Set the data dir
    builder = builder.data_dir(PathBuf::from("/tmp/helios"));

    // Set the fallback service
    builder = builder.fallback("https://sync-mainnet.beaconcha.in");

    // Enable lazy checkpoints
    builder = builder.load_external_fallback();

    // Build the client
    let _client: Client = builder.build().unwrap();
    println!("Constructed client!");

    Ok(())
}
