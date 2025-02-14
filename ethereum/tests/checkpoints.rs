use alloy::primitives::B256;
use helios_ethereum::config::{checkpoints, networks};

#[tokio::test]
async fn test_checkpoint_fallback() {
    let cf = checkpoints::CheckpointFallback::new();

    assert_eq!(cf.services.get(&networks::Network::Mainnet), None);
    assert_eq!(cf.services.get(&networks::Network::Sepolia), None);
    assert_eq!(cf.services.get(&networks::Network::Holesky), None);

    assert_eq!(
        cf.networks,
        [
            networks::Network::Mainnet,
            networks::Network::Sepolia,
            networks::Network::Holesky,
        ]
        .to_vec()
    );
}

#[tokio::test]
async fn test_construct_checkpoints() {
    let cf = checkpoints::CheckpointFallback::new()
        .build()
        .await
        .unwrap();

    assert!(cf.services[&networks::Network::Mainnet].len() > 1);
    assert!(cf.services[&networks::Network::Sepolia].len() > 1);
    assert!(cf.services[&networks::Network::Holesky].len() > 1);
}

#[tokio::test]
async fn test_fetch_latest_checkpoints() {
    let cf = checkpoints::CheckpointFallback::new()
        .build()
        .await
        .unwrap();
    let checkpoint = cf
        .fetch_latest_checkpoint(&networks::Network::Sepolia)
        .await
        .unwrap();
    assert!(checkpoint != B256::ZERO);
    let checkpoint = cf
        .fetch_latest_checkpoint(&networks::Network::Holesky)
        .await
        .unwrap();
    assert!(checkpoint != B256::ZERO);
    let checkpoint = cf
        .fetch_latest_checkpoint(&networks::Network::Mainnet)
        .await
        .unwrap();
    assert!(checkpoint != B256::ZERO);
}

#[tokio::test]
async fn test_get_all_fallback_endpoints() {
    let cf = checkpoints::CheckpointFallback::new()
        .build()
        .await
        .unwrap();
    let urls = cf.get_all_fallback_endpoints(&networks::Network::Mainnet);
    assert!(!urls.is_empty());
    let urls = cf.get_all_fallback_endpoints(&networks::Network::Sepolia);
    assert!(!urls.is_empty());
    let urls = cf.get_all_fallback_endpoints(&networks::Network::Holesky);
    assert!(!urls.is_empty());
}

#[tokio::test]
async fn test_get_healthy_fallback_endpoints() {
    let cf = checkpoints::CheckpointFallback::new()
        .build()
        .await
        .unwrap();
    let urls = cf.get_healthy_fallback_endpoints(&networks::Network::Mainnet);
    assert!(!urls.is_empty());
    let urls = cf.get_healthy_fallback_endpoints(&networks::Network::Sepolia);
    assert!(!urls.is_empty());
    let urls = cf.get_healthy_fallback_endpoints(&networks::Network::Holesky);
    assert!(!urls.is_empty());
}
