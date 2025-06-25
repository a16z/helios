use std::str::FromStr;
use std::time::{Duration, Instant};

use alloy::eips::BlockNumberOrTag;
use alloy::primitives::Address;
use criterion::{criterion_group, criterion_main, Criterion};

mod harness;

criterion_main!(benchmarks);
criterion_group! {
    name = benchmarks;
    config = Criterion::default().sample_size(5);
    targets = bench_balance_comparison
}

pub fn bench_balance_comparison(c: &mut Criterion) {
    c.bench_function("balance_helios_vs_rpc", |b| {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        // Set up clients
        let execution_rpc =
            std::env::var("MAINNET_EXECUTION_RPC").expect("MAINNET_EXECUTION_RPC must be set");

        let helios_client = rt.block_on(async {
            let checkpoint = harness::fetch_mainnet_checkpoint().await.unwrap();
            harness::construct_mainnet_client_with_checkpoint(checkpoint)
                .await
                .unwrap()
        });

        let helios_client = std::sync::Arc::new(helios_client);

        // Test address
        let addr = Address::from_str("0x00000000219ab540356cbb839cbe05303d7705fa").unwrap();
        let block = BlockNumberOrTag::Latest;

        // Benchmark
        b.to_async(rt).iter(|| async {
            let client = std::sync::Arc::clone(&helios_client);

            // Time Helios
            let start = Instant::now();
            let _balance = client.get_balance(addr, block.into()).await.unwrap();
            let helios_time = start.elapsed();

            // For now, just return the time
            // In a full implementation, we'd also query the RPC directly
            helios_time
        })
    });
}
