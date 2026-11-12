#![cfg(not(target_arch = "wasm32"))]

use std::time::Duration;

use alloy::{eips::BlockNumberOrTag, primitives::B256, rpc::types::Filter};
use axum::{extract::OriginalUri, routing::get, Json, Router};
use helios_ethereum::spec::Ethereum;
use helios_verifiable_api_client::{http::HttpVerifiableApi, VerifiableApi};
use tokio::{net::TcpListener, sync::mpsc, time::timeout};
use url::Url;

async fn assert_logs_query(filter: Filter, expected: &[(&str, &str)]) {
    let (sender, mut receiver) = mpsc::channel(1);
    let app = Router::new().route(
        "/eth/v1/proof/logs",
        get(move |OriginalUri(uri): OriginalUri| {
            let sender = sender.clone();
            async move {
                sender.send(uri).await.unwrap();
                Json(serde_json::json!({ "logs": [], "receiptProofs": {} }))
            }
        }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base_url = Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = HttpVerifiableApi::<Ethereum>::new(&base_url);
    let result = timeout(Duration::from_secs(5), client.get_logs(&filter)).await;
    server.abort();
    let response = result.expect("log request timed out").unwrap();
    assert!(response.logs.is_empty());
    assert!(response.receipt_proofs.is_empty());

    let uri = receiver
        .try_recv()
        .expect("server did not receive log request");
    assert_eq!(uri.path(), "/eth/v1/proof/logs");
    let request_url = base_url.join(&uri.to_string()).unwrap();
    let mut actual: Vec<_> = request_url.query_pairs().into_owned().collect();
    actual.sort();
    let mut expected: Vec<_> = expected
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect();
    expected.sort();
    assert_eq!(actual, expected);
}

#[tokio::test]
async fn preserves_block_tags_in_both_range_bounds() {
    for (tag, value) in [
        (BlockNumberOrTag::Earliest, "earliest"),
        (BlockNumberOrTag::Latest, "latest"),
        (BlockNumberOrTag::Safe, "safe"),
        (BlockNumberOrTag::Finalized, "finalized"),
        (BlockNumberOrTag::Pending, "pending"),
    ] {
        assert_logs_query(Filter::new().from_block(tag), &[("fromBlock", value)]).await;
        assert_logs_query(Filter::new().to_block(tag), &[("toBlock", value)]).await;
    }
}

#[tokio::test]
async fn preserves_earliest_to_finalized_range() {
    assert_logs_query(
        Filter::new()
            .from_block(BlockNumberOrTag::Earliest)
            .to_block(BlockNumberOrTag::Finalized),
        &[("fromBlock", "earliest"), ("toBlock", "finalized")],
    )
    .await;
}

#[tokio::test]
async fn preserves_numeric_and_mixed_ranges() {
    assert_logs_query(
        Filter::new().from_block(0).to_block(42),
        &[("fromBlock", "0x0"), ("toBlock", "0x2a")],
    )
    .await;
    assert_logs_query(
        Filter::new()
            .from_block(42)
            .to_block(BlockNumberOrTag::Safe),
        &[("fromBlock", "0x2a"), ("toBlock", "safe")],
    )
    .await;
    assert_logs_query(
        Filter::new()
            .from_block(BlockNumberOrTag::Earliest)
            .to_block(42),
        &[("fromBlock", "earliest"), ("toBlock", "0x2a")],
    )
    .await;
}

#[tokio::test]
async fn preserves_block_hash_without_range_bounds() {
    let hash = B256::repeat_byte(0x11);
    assert_logs_query(
        Filter::new().at_block_hash(hash),
        &[("blockHash", &hash.to_string())],
    )
    .await;
}

#[tokio::test]
async fn omits_unspecified_range_bounds() {
    assert_logs_query(Filter::new(), &[]).await;
}
