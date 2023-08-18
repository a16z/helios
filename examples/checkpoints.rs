use eyre::Result;

// From helios::config
use config::{checkpoints, networks};

#[tokio::main]
async fn main() -> Result<()> {
    // Construct the checkpoint fallback services.
    // The `build` method will fetch a list of [CheckpointFallbackService]s from a community-mainained list by ethPandaOps.
    // This list is NOT guaranteed to be secure, but is provided in good faith.
    // The raw list can be found here: https://github.com/ethpandaops/checkpoint-sync-health-checks/blob/master/_data/endpoints.yaml
    let cf = checkpoints::CheckpointFallback::new()
        .build()
        .await
        .unwrap();

    // Fetch the latest goerli checkpoint
    let goerli_checkpoint = cf
        .fetch_latest_checkpoint(&networks::Network::GOERLI)
        .await
        .unwrap();
    println!("Fetched latest goerli checkpoint: {goerli_checkpoint}");

    // Let's get a list of all the fallback service endpoints for goerli
    let endpoints = cf.get_all_fallback_endpoints(&networks::Network::GOERLI);
    println!("Fetched all goerli fallback endpoints: {endpoints:#?}");

    // Since we built the checkpoint fallback services, we can also just get the raw checkpoint fallback services.
    // The `get_fallback_services` method returns a reference to the internal list of CheckpointFallbackService objects
    // for the given network.
    let services = cf.get_fallback_services(&networks::Network::GOERLI);
    println!("Fetched all goerli fallback services: {services:#?}");

    // Fetch the latest mainnet checkpoint
    let mainnet_checkpoint = cf
        .fetch_latest_checkpoint(&networks::Network::MAINNET)
        .await
        .unwrap();
    println!("Fetched latest mainnet checkpoint: {mainnet_checkpoint}");

    // Let's get a list of all the fallback service endpoints for mainnet
    let endpoints = cf.get_all_fallback_endpoints(&networks::Network::MAINNET);
    println!("Fetched all mainnet fallback endpoints: {endpoints:#?}");

    // Since we built the checkpoint fallback services, we can also just get the raw checkpoint fallback services.
    // The `get_fallback_services` method returns a reference to the internal list of CheckpointFallbackService objects
    // for the given network.
    let services = cf.get_fallback_services(&networks::Network::MAINNET);
    println!("Fetched all mainnet fallback services: {services:#?}");

    Ok(())
}
