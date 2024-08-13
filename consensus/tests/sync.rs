use std::sync::Arc;

use config::{networks, Config};
use consensus::{database::ConfigDB, rpc::mock_rpc::MockRpc, ConsensusClient};

async fn setup() -> ConsensusClient<MockRpc, ConfigDB> {
    let base_config = networks::mainnet();
    let config = Config {
        consensus_rpc: String::new(),
        execution_rpc: String::new(),
        chain: base_config.chain,
        forks: base_config.forks,
        max_checkpoint_age: 123123123,
        checkpoint: Some(
            hex::decode("5afc212a7924789b2bc86acad3ab3a6ffb1f6e97253ea50bee7f4f51422c9275")
                .unwrap(),
        ),
        ..Default::default()
    };

    ConsensusClient::new("testdata/", Arc::new(config)).unwrap()
}

#[tokio::test]
async fn test_sync() {
    let client = setup().await;

    let block = client.block_recv.unwrap().recv().await.unwrap();
    assert_eq!(block.number.to::<u64>(), 17923113);
}
