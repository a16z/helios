use std::sync::Arc;

use alloy::primitives::b256;
use url::Url;

use helios_consensus_core::consensus_spec::MainnetConsensusSpec;
use helios_ethereum::config::{networks, Config};
use helios_ethereum::{consensus::ConsensusClient, database::ConfigDB, rpc::mock_rpc::MockRpc};

async fn setup() -> ConsensusClient<MainnetConsensusSpec, MockRpc, ConfigDB> {
    let base_config = networks::mainnet();
    let config = Config {
        consensus_rpc: Url::parse("http://localhost:8545").unwrap(),
        chain: base_config.chain,
        forks: base_config.forks,
        max_checkpoint_age: 123123123,
        checkpoint: Some(b256!(
            "5afc212a7924789b2bc86acad3ab3a6ffb1f6e97253ea50bee7f4f51422c9275"
        )),
        ..Default::default()
    };

    let url = Url::parse("file://testdata/").unwrap();
    ConsensusClient::new(&url, Arc::new(config)).unwrap()
}

#[tokio::test]
async fn test_sync() {
    let client = setup().await;

    let block = client.block_recv.unwrap().recv().await.unwrap();
    assert_eq!(block.header.number, 17923112_u64);
}
