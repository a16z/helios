use std::path::PathBuf;
use dirs::home_dir;

use eyre::Result;

use helios::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Create a new Helios Client Builder
    let mut builder = ClientBuilder::new();

    // Set the network to goerli
    builder = builder.network(networks::Network::GOERLI);

    let execution_rpc_url = std::env::var("GOERLI_EXECUTION_RPC")?;
    let consensus_rpc_url = std::env::var("GOERLI_CONSENSUS_RPC")?;

    // Set the consensus rpc url
    builder = builder.consensus_rpc(&consensus_rpc_url);

    // Set the execution rpc url
    builder = builder.execution_rpc(&execution_rpc_url);

    // Set the checkpoint to the last known checkpoint. See config.md
    builder =
        builder.checkpoint("7beab8f82587b1e9f2079beddebde49c2ed5c0da4ce86ea22de6a6b2dc7aa86b");

    // Set the rpc port
    builder = builder.rpc_port(8545);

    // Set the data dir
    let data_path = home_dir().unwrap().join(".helios/data/goerli");
    builder = builder.data_dir(PathBuf::from(data_path));

    // Set the fallback service. See config.md
    // builder = builder.fallback("https://sync-goerli.beaconcha.in");

    // Enable lazy checkpoints
    // builder = builder.load_external_fallback();

    // builder = builder.strict_checkpoint_age();

    // Build the client
    let _client: Client<FileDB> = builder.build().unwrap();
    println!("Constructed client!");

    Ok(())
}
