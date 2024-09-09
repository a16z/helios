use alloy::primitives::Address;
use criterion::{criterion_group, criterion_main, Criterion};
use helios::types::BlockTag;
use std::str::FromStr;

mod harness;

criterion_main!(get_balance);
criterion_group! {
    name = get_balance;
    config = Criterion::default().sample_size(10);
    targets = bench_mainnet_get_balance, bench_goerli_get_balance
}

/// Benchmark mainnet get balance.
/// Address: 0x00000000219ab540356cbb839cbe05303d7705fa (beacon chain deposit address)
pub fn bench_mainnet_get_balance(c: &mut Criterion) {
    c.bench_function("get_balance", |b| {
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

/// Benchmark goerli get balance.
/// Address: 0xB4FBF271143F4FBf7B91A5ded31805e42b2208d6 (goerli weth)
pub fn bench_goerli_get_balance(c: &mut Criterion) {
    c.bench_function("get_balance", |b| {
        // Create a new multi-threaded tokio runtime.
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        // Construct a goerli client using our harness and tokio runtime.
        let gc = match harness::construct_goerli_client(&rt) {
            Ok(gc) => gc,
            Err(e) => {
                println!("failed to construct goerli client: {}", e);
                std::process::exit(1);
            }
        };
        let client = std::sync::Arc::new(gc);

        // Get the beacon chain deposit contract address.
        let addr = Address::from_str("0xB4FBF271143F4FBf7B91A5ded31805e42b2208d6").unwrap();
        let block = BlockTag::Latest;

        // Execute the benchmark asynchronously.
        b.to_async(rt).iter(|| async {
            let inner = std::sync::Arc::clone(&client);
            inner.get_balance(addr, block).await.unwrap()
        })
    });
}
