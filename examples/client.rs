use std::path::PathBuf;
use dirs::home_dir;

use eyre::Result;

use helios::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    // Create a new Helios Client Builder
    let mut builder = ClientBuilder::new();

    // Set the network to mainnet
    builder = builder.network(networks::Network::MAINNET);

    let execution_rpc_url = std::env::var("MAINNET_EXECUTION_RPC")?;
    let consensus_rpc_url = std::env::var("MAINNET_CONSENSUS_RPC")?;

    // Set the consensus rpc url
    builder = builder.consensus_rpc(&consensus_rpc_url);

    // Set the execution rpc url
    builder = builder.execution_rpc(&execution_rpc_url);

    let mainnet_checkpoint = std::env::var("MAINNET_CHECKPOINT")?;
    let mut mainnet_checkpoint_stripped = "";
    if let Some(stripped) = mainnet_checkpoint.strip_prefix("0x") {
        mainnet_checkpoint_stripped = stripped;
    }

    // Set the checkpoint to the last known checkpoint. See config.md
    builder = builder.checkpoint(mainnet_checkpoint_stripped);

    // Set the rpc port
    builder = builder.rpc_port(8545);

    // Set the data dir

    // .helios/mainnet
    let mainnet_data_dir_ext = std::env::var("MAINNET_DATA_DIR_EXT")?;
    let data_path = home_dir().unwrap().join(mainnet_data_dir_ext);
    builder = builder.data_dir(PathBuf::from(data_path));

    // Set the fallback service. See config.md
    let mainnet_checkpoint_fallback = std::env::var("MAINNET_CHECKPOINT_FALLBACK")?;
    builder = builder.fallback(&mainnet_checkpoint_fallback);

    // Enable lazy checkpoints
    builder = builder.load_external_fallback();

    // Set string checkpoint age
    builder = builder.strict_checkpoint_age();

    // Build the client
    let _client: Client = builder.build().unwrap();
    println!("Constructed client!");

    Ok(())
}
