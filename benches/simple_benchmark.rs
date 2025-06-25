// Simplified benchmark that measures Helios vs RPC performance
use std::str::FromStr;
use std::time::Instant;

use alloy::eips::BlockNumberOrTag;
use alloy::primitives::Address;

mod harness;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    println!("Helios vs RPC Benchmark\n");

    // Configuration
    let execution_rpc = std::env::var("MAINNET_EXECUTION_RPC")?;
    let runs = 5;

    println!("Configuration:");
    println!("  Execution RPC: {}", execution_rpc);
    println!("  Runs: {}\n", runs);

    // Initialize Helios client
    println!("Initializing Helios client...");
    let checkpoint = harness::fetch_mainnet_checkpoint().await?;
    let helios_client = harness::construct_mainnet_client_with_checkpoint(checkpoint).await?;
    println!("Helios client synced!\n");

    // Test parameters
    let test_address = Address::from_str("0x00000000219ab540356cbb839cbe05303d7705fa")?;
    let block = BlockNumberOrTag::Latest;

    // Run benchmarks
    println!("Running balance fetch benchmark...");
    let mut helios_times = Vec::new();

    for i in 0..runs {
        print!("  Run {}/{}... ", i + 1, runs);

        // Measure Helios
        let start = Instant::now();
        let balance = helios_client
            .get_balance(test_address, block.into())
            .await?;
        let helios_time = start.elapsed();
        helios_times.push(helios_time);

        println!("done ({}ms)", helios_time.as_millis());
    }

    // Calculate average
    let avg_time = helios_times.iter().map(|d| d.as_millis()).sum::<u128>() / runs as u128;

    println!("\nResults:");
    println!("  Average Helios time: {}ms", avg_time);

    Ok(())
}
