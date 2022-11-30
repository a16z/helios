use ethers::types::H256;

#[tokio::test]
async fn test_construct_checkpoints() {
    let mut list = config::checkpoints::CheckpointFallbackList::new();
    list.construct().await.unwrap();

    assert!(list.mainnet.len() > 1);
    assert!(list.goerli.len() > 1);
}

#[tokio::test]
async fn test_fetch_latest_mainnet_checkpoints() {
    let mut list = config::checkpoints::CheckpointFallbackList::new();
    list.construct().await.unwrap();
    let checkpoint = list.fetch_latest_mainnet_checkpoint().await.unwrap();
    assert!(checkpoint != H256::zero());
}

#[tokio::test]
async fn test_fetch_latest_goerli_checkpoints() {
    let mut list = config::checkpoints::CheckpointFallbackList::new();
    list.construct().await.unwrap();
    let checkpoint = list.fetch_latest_goerli_checkpoint().await.unwrap();
    assert!(checkpoint != H256::zero());
}
