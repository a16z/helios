use criterion::{criterion_group, criterion_main, Criterion};
use ethers::types::Address;
use helios::types::BlockTag;

mod harness;

criterion_main!(sync);
criterion_group! {
    name = sync;
    config = Criterion::default().sample_size(10);
    targets =
        bench_full_sync,
        bench_full_sync_with_call,
        bench_full_sync_checkpoint_fallback,
        bench_full_sync_with_call_checkpoint_fallback,
}

/// Benchmark full client sync.
pub fn bench_full_sync(c: &mut Criterion) {
    // Externally, let's fetch the latest checkpoint from our fallback service so as not to
    // benchmark the checkpoint fetch.
    let checkpoint = harness::await_future(harness::fetch_mainnet_checkpoint()).unwrap();
    let checkpoint = hex::encode(checkpoint);

    // On client construction, it will sync to the latest checkpoint using our fetched checkpoint.
    c.bench_function("full_sync", |b| {
        b.to_async(harness::construct_runtime()).iter(|| async {
            let _client = std::sync::Arc::new(
                harness::construct_mainnet_client_with_checkpoint(&checkpoint)
                    .await
                    .unwrap(),
            );
        })
    });
}

/// Benchmark full client sync.
/// Address: 0x00000000219ab540356cbb839cbe05303d7705fa (beacon chain deposit address)
pub fn bench_full_sync_with_call(c: &mut Criterion) {
    // Externally, let's fetch the latest checkpoint from our fallback service so as not to
    // benchmark the checkpoint fetch.
    let checkpoint = harness::await_future(harness::fetch_mainnet_checkpoint()).unwrap();
    let checkpoint = hex::encode(checkpoint);

    // On client construction, it will sync to the latest checkpoint using our fetched checkpoint.
    c.bench_function("full_sync_call", |b| {
        b.to_async(harness::construct_runtime()).iter(|| async {
            let client = std::sync::Arc::new(
                harness::construct_mainnet_client_with_checkpoint(&checkpoint)
                    .await
                    .unwrap(),
            );
            let addr = "0x00000000219ab540356cbb839cbe05303d7705fa"
                .parse::<Address>()
                .unwrap();
            let block = BlockTag::Latest;
            client.get_balance(&addr, block).await.unwrap()
        })
    });
}

/// Benchmark full client sync with checkpoint fallback.
pub fn bench_full_sync_checkpoint_fallback(c: &mut Criterion) {
    c.bench_function("full_sync_fallback", |b| {
        let rt = harness::construct_runtime();
        b.iter(|| {
            let _client = std::sync::Arc::new(harness::construct_mainnet_client(&rt).unwrap());
        })
    });
}

/// Benchmark full client sync with a call and checkpoint fallback.
/// Address: 0x00000000219ab540356cbb839cbe05303d7705fa (beacon chain deposit address)
pub fn bench_full_sync_with_call_checkpoint_fallback(c: &mut Criterion) {
    c.bench_function("full_sync_call", |b| {
        let addr = "0x00000000219ab540356cbb839cbe05303d7705fa";
        let rt = harness::construct_runtime();
        b.iter(|| {
            let client = harness::construct_mainnet_client(&rt).unwrap();
            harness::get_balance(&rt, client, addr).unwrap();
        })
    });
}
