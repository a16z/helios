use std::sync::Arc;

use config::{networks, Config};
use consensus::{rpc::mock_rpc::MockRpc, ConsensusClient};

async fn setup() -> ConsensusClient<MockRpc> {
    let base_config = networks::goerli();
    let config = Config {
        consensus_rpc: String::new(),
        execution_rpc: String::new(),
        chain: base_config.chain,
        forks: base_config.forks,
        ..Default::default()
    };

    ConsensusClient::new("testdata/", &base_config.checkpoint, Arc::new(config))
        .await
        .unwrap()
}

#[tokio::test]
async fn test_sync() {
    let mut client = setup().await;
    client.sync().await.unwrap();

    let head = client.get_header();
    assert_eq!(head.slot, 3818196);

    let finalized_head = client.get_finalized_header();
    assert_eq!(finalized_head.slot, 3818112);
}

#[tokio::test]
async fn test_get_payload() {
    let mut client = setup().await;
    client.sync().await.unwrap();

    let payload = client.get_execution_payload(&None).await.unwrap();
    assert_eq!(payload.block_number, 7530932);
}
