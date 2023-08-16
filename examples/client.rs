use std::path::PathBuf;

use eyre::Result;

use helios::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Create a new Helios Client Builder
    let mut builder = ClientBuilder::new();

    // Set the network to goerli
    builder = builder.network(networks::Network::GOERLI);

    // Set the consensus rpc url
    builder = builder.consensus_rpc("https://www.lightclientdata.org");

    // Set the execution rpc url
    builder = builder.execution_rpc("https://ethereum-goerli-rpc.allthatnode.com");

    // Set the checkpoint to the last known checkpoint. See config.md
    builder =
        builder.checkpoint("b5c375696913865d7c0e166d87bc7c772b6210dc9edf149f4c7ddc6da0dd4495");

    // Set the rpc port
    builder = builder.rpc_port(8545);

    // Set the data dir
    builder = builder.data_dir(PathBuf::from("/tmp/helios"));

    // Set the fallback service. See config.md
    builder = builder.fallback("https://sync-goerli.beaconcha.in");

    // Enable lazy checkpoints
    builder = builder.load_external_fallback();

    // Build the client
    let _client: Client<FileDB> = builder.build().unwrap();
    println!("Constructed client!");

    Ok(())
}
