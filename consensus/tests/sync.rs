use std::sync::Arc;

use config::{networks, Config};
use consensus::{rpc::mock_rpc::MockRpc, ConsensusClient};

async fn setup() -> ConsensusClient<MockRpc> {
    let base_config = networks::mainnet();
    let config = Config {
        consensus_rpc: String::new(),
        execution_rpc: String::new(),
        chain: base_config.chain,
        forks: base_config.forks,
        max_checkpoint_age: 123123123,
        ..Default::default()
    };

    let checkpoint =
        hex::decode("5afc212a7924789b2bc86acad3ab3a6ffb1f6e97253ea50bee7f4f51422c9275").unwrap();

    ConsensusClient::new("testdata/", &checkpoint, Arc::new(config)).unwrap()
}

#[tokio::test]
async fn test_sync() {
    let mut client = setup().await;

    let payload = client.payload_recv.recv().await.unwrap();
    assert_eq!(payload.block_number().as_u64(), 17923113);
}
