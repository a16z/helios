#[tokio::test]
async fn test_construct_checkpoints() {
    let mut list = config::checkpoints::CheckpointFallbackList::new();
    list.construct().await.unwrap();

    assert!(list.mainnet.len() > 1);
    assert!(list.goerli.len() > 1);
}
