use std::sync::Arc;

use config::networks;
use consensus::{rpc::mock_rpc::MockRpc, ConsensusClient};

async fn setup() -> ConsensusClient<MockRpc> {
    ConsensusClient::new(
        "testdata/",
        &hex::decode("1e591af1e90f2db918b2a132991c7c2ee9a4ab26da496bd6e71e4f0bd65ea870").unwrap(),
        Arc::new(networks::goerli()),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn test_sync() {
    let mut client = setup().await;
    client.sync().await.unwrap();

    let head = client.get_header();
    assert!(head.slot == 3818196);

    let finalized_head = client.get_finalized_header();
    assert!(finalized_head.slot == 3818112);
}

#[tokio::test]
async fn test_get_payload() {
    let mut client = setup().await;
    client.sync().await.unwrap();

    let payload = client.get_execution_payload(&None).await.unwrap();
    assert!(payload.block_number == 7530932);
}
