use std::path::PathBuf;

use alloy::primitives::b256;
use eyre::Result;

use helios::ethereum::{config::networks::Network, EthereumClient, EthereumClientBuilder};

#[tokio::main]
async fn main() -> Result<()> {
    // Create a new Helios Client Builder
    let builder = EthereumClientBuilder::new()
        // Set the network to mainnet
        .network(Network::Mainnet)
        // Set the consensus rpc url
        .consensus_rpc("https://www.lightclientdata.org")?
        // Set the execution rpc url
        .execution_rpc("https://eth-mainnet.g.alchemy.com/v2/XXXXX")?
        // Set the checkpoint to the last known checkpoint
        .checkpoint(b256!(
            "85e6151a246e8fdba36db27a0c7678a575346272fe978c9281e13a8b26cdfa68"
        ))
        // Set the rpc address
        .rpc_address("127.0.0.1:8545".parse().unwrap())
        // Set the data dir
        .data_dir(PathBuf::from("/tmp/helios"))
        // Set the fallback service
        .fallback("https://sync-mainnet.beaconcha.in")?
        // Enable lazy checkpoints
        .load_external_fallback()
        // Select the FileDB
        .with_file_db();

    // Build the client
    let client: EthereumClient = builder.build().unwrap();
    println!("Constructed client!");

    match client.wait_synced().await {
        Ok(()) => println!("Synced!"),
        Err(err) => println!("Failure syncing - {err:?}"),
    }

    Ok(())
}
