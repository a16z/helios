use std::str::FromStr;

use alloy::primitives::Address;
use criterion::{criterion_group, criterion_main, Criterion};

use helios_common::types::BlockTag;

mod harness;

criterion_main!(get_balance);
criterion_group! {
    name = get_balance;
    config = Criterion::default().sample_size(10);
    targets = bench_mainnet_get_balance, bench_sepolia_get_balance
}

/// Benchmark mainnet get balance.
/// Address: 0x00000000219ab540356cbb839cbe05303d7705fa (beacon chain deposit address)
pub fn bench_mainnet_get_balance(c: &mut Criterion) {
    c.bench_function("get_balance_mainnet", |b| {
        // Create a new multi-threaded tokio runtime.
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        // Construct a mainnet client using our harness and tokio runtime.
        let client = std::sync::Arc::new(harness::construct_mainnet_client(&rt).unwrap());

        // Get the beacon chain deposit contract address.
        let addr = Address::from_str("0x00000000219ab540356cbb839cbe05303d7705fa").unwrap();
        let block = BlockTag::Latest;

        // Execute the benchmark asynchronously.
        b.to_async(rt).iter(|| async {
            let inner = std::sync::Arc::clone(&client);
            inner.get_balance(addr, block).await.unwrap()
        })
    });
}

/// Benchmark sepolia get balance.
/// Address: 0x7b79995e5f793a07bc00c21412e50ecae098e7f9 (sepolia weth)
pub fn bench_sepolia_get_balance(c: &mut Criterion) {
    c.bench_function("get_balance_sepolia", |b| {
        // Create a new multi-threaded tokio runtime.
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        // Construct a sepolia client using our harness and tokio runtime.
        let client = std::sync::Arc::new(
            harness::construct_sepolia_client(&rt)
                .expect("failed to construct sepolia client for benchmark"),
        );

        // Get the beacon chain deposit contract address.
        let addr = Address::from_str("0x7b79995e5f793a07bc00c21412e50ecae098e7f9").unwrap();
        let block = BlockTag::Latest;

        // Execute the benchmark asynchronously.
        b.to_async(rt).iter(|| async {
            let inner = std::sync::Arc::clone(&client);
            inner.get_balance(addr, block).await.unwrap()
        })
    });
}
