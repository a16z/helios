use alloy::primitives::B256;
use helios_ethereum::config::{checkpoints, networks};

fn checkpoint_fallback() -> checkpoints::CheckpointFallback {
    // Live tests only require checkpoint services for active networks.
    checkpoints::CheckpointFallback {
        networks: vec![networks::Network::Mainnet, networks::Network::Sepolia],
        ..Default::default()
    }
}

#[tokio::test]
async fn test_checkpoint_fallback() {
    let cf = checkpoints::CheckpointFallback::new();

    assert!(cf.services.is_empty());
    assert!(cf.networks.contains(&networks::Network::Mainnet));
    assert!(cf.networks.contains(&networks::Network::Sepolia));
}

#[tokio::test]
async fn test_construct_checkpoints() {
    let cf = checkpoint_fallback().build().await.unwrap();

    assert!(cf.services[&networks::Network::Mainnet].len() > 1);
    assert!(cf.services[&networks::Network::Sepolia].len() > 1);
}

#[tokio::test]
async fn test_fetch_latest_checkpoints() {
    let cf = checkpoint_fallback().build().await.unwrap();
    let checkpoint = cf
        .fetch_latest_checkpoint(&networks::Network::Sepolia)
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
    let cf = checkpoint_fallback().build().await.unwrap();
    let urls = cf.get_all_fallback_endpoints(&networks::Network::Mainnet);
    assert!(!urls.is_empty());
    let urls = cf.get_all_fallback_endpoints(&networks::Network::Sepolia);
    assert!(!urls.is_empty());
}

#[tokio::test]
async fn test_get_healthy_fallback_endpoints() {
    let cf = checkpoint_fallback().build().await.unwrap();
    let urls = cf.get_healthy_fallback_endpoints(&networks::Network::Mainnet);
    assert!(!urls.is_empty());
    let urls = cf.get_healthy_fallback_endpoints(&networks::Network::Sepolia);
    assert!(!urls.is_empty());
}
