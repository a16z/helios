use alloy::primitives::Address;
use eyre::Result;
use helios_ethereum::EthereumClientBuilder;
use std::path::PathBuf;
use std::str::FromStr;

mod framework;
mod harness;

use framework::{proxy, Benchmark, BenchmarkCase, BenchmarkReport};

#[tokio::main]
async fn main() -> Result<()> {
    println!("Starting Helios Benchmarking Suite...\n");

    let execution_rpc = std::env::var("MAINNET_EXECUTION_RPC")
        .expect("MAINNET_EXECUTION_RPC environment variable must be set");

    let standard_rpc =
        std::env::var("BENCHMARK_STANDARD_RPC").unwrap_or_else(|_| execution_rpc.clone());

    let runs = std::env::var("BENCHMARK_RUNS")
        .unwrap_or_else(|_| "5".to_string())
        .parse::<usize>()?;

    println!("Configuration:");
    println!("  Execution RPC: {}", execution_rpc);
    println!("  Standard RPC: {}", standard_rpc);
    println!("  Runs per benchmark: {}\n", runs);

    println!("Starting proxy services...");
    let (execution_proxy, standard_proxy) =
        proxy::start_proxy_pair(&execution_rpc, &standard_rpc).await?;
    println!("  Execution proxy started at: {}", execution_proxy.url);
    println!("  Standard proxy started at: {}\n", standard_proxy.url);

    println!("Initializing Helios client...");
    let helios_client = EthereumClientBuilder::new()
        .network(helios_ethereum::config::networks::Network::Mainnet)
        .consensus_rpc("https://www.lightclientdata.org")?
        .execution_rpc(&execution_proxy.url)?
        .strict_checkpoint_age()
        .load_external_fallback()
        .data_dir(PathBuf::from("/tmp/helios-bench"))
        .with_file_db()
        .build()?;

    println!("  Waiting for Helios to sync...");
    helios_client.wait_synced().await;
    println!("  Helios client synced!\n");

    let mut benchmark =
        Benchmark::new(helios_client, execution_proxy, standard_proxy, runs).await?;

    let beacon_deposit_address = Address::from_str("0x00000000219ab540356cbb839cbe05303d7705fa")?;
    let usdc_address = Address::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")?;
    let vitalik_address = Address::from_str("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045")?;

    let cases = vec![
        BenchmarkCase::EthBalance {
            address: beacon_deposit_address,
        },
        BenchmarkCase::ContractCall {
            contract: usdc_address,
            method_sig: "balanceOf(address)".to_string(),
            args: serde_json::json!([vitalik_address]),
        },
        BenchmarkCase::BlockFetch { historical: false },
        BenchmarkCase::BlockFetch { historical: true },
        BenchmarkCase::ReceiptFetch { historical: false },
        BenchmarkCase::ReceiptFetch { historical: true },
        BenchmarkCase::TransactionFetch { historical: false },
        BenchmarkCase::TransactionFetch { historical: true },
        BenchmarkCase::LogsFetch {
            filter: serde_json::json!({
                "fromBlock": "latest",
                "toBlock": "latest",
                "address": usdc_address,
                "topics": []
            }),
        },
    ];

    println!("Running benchmarks...");
    let mut results = Vec::new();

    for (i, case) in cases.iter().enumerate() {
        println!("  [{}/{}] Running: {}", i + 1, cases.len(), case.name());
        match benchmark.run_case(case.clone()).await {
            Ok(result) => {
                println!("    Completed successfully");
                results.push(result);
            }
            Err(e) => {
                eprintln!("    Error: {:?}", e);
                continue;
            }
        }
    }

    let report = BenchmarkReport::new(results);
    report.print();
    println!("{}", report.summary());

    Ok(())
}
