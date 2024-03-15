use config::networks;
use ethers::types::H256;

#[tokio::test]
async fn test_checkpoint_fallback() {
    let cf = config::checkpoints::CheckpointFallback::new();

    assert_eq!(cf.services.get(&networks::Network::MAINNET), None);
    assert_eq!(cf.services.get(&networks::Network::GOERLI), None);

    assert_eq!(
        cf.networks,
        [
            networks::Network::MAINNET,
            networks::Network::GOERLI,
            networks::Network::SEPOLIA
        ]
        .to_vec()
    );
}

#[tokio::test]
async fn test_construct_checkpoints() {
    let cf = config::checkpoints::CheckpointFallback::new()
        .build()
        .await
        .unwrap();

    assert!(cf.services[&networks::Network::MAINNET].len() > 1);
    assert!(cf.services[&networks::Network::GOERLI].len() > 1);
}

#[tokio::test]
async fn test_fetch_latest_checkpoints() {
    let cf = config::checkpoints::CheckpointFallback::new()
        .build()
        .await
        .unwrap();
    let checkpoint = cf
        .fetch_latest_checkpoint(&networks::Network::GOERLI)
        .await
        .unwrap();
    assert!(checkpoint != H256::zero());
    let checkpoint = cf
        .fetch_latest_checkpoint(&networks::Network::MAINNET)
        .await
        .unwrap();
    assert!(checkpoint != H256::zero());
}

#[tokio::test]
async fn test_get_all_fallback_endpoints() {
    let cf = config::checkpoints::CheckpointFallback::new()
        .build()
        .await
        .unwrap();
    let urls = cf.get_all_fallback_endpoints(&networks::Network::MAINNET);
    assert!(!urls.is_empty());
    let urls = cf.get_all_fallback_endpoints(&networks::Network::GOERLI);
    assert!(!urls.is_empty());
}

#[tokio::test]
async fn test_get_healthy_fallback_endpoints() {
    let cf = config::checkpoints::CheckpointFallback::new()
        .build()
        .await
        .unwrap();
    let urls = cf.get_healthy_fallback_endpoints(&networks::Network::MAINNET);
    assert!(!urls.is_empty());
    let urls = cf.get_healthy_fallback_endpoints(&networks::Network::GOERLI);
    assert!(!urls.is_empty());
}
