use alloy::eips::{eip1898::RpcBlockHash, BlockId};
use helios_common::execution_provider::BlockProvider;
use helios_ethereum::spec::Ethereum;
use helios_test_utils::rpc_block;

use helios_core::execution::{
    constants::MAX_STATE_HISTORY_LENGTH, providers::block::block_cache::BlockCache,
};

#[tokio::test]
async fn reorg_does_not_resolve_old_hash_to_new_block() {
    let cache = BlockCache::<Ethereum>::new();
    let old = rpc_block();
    let mut parent = old.clone();
    parent.header.number -= 1;
    parent.header.hash = old.header.parent_hash;
    cache.push_block(parent, BlockId::latest()).await;
    cache.push_block(old.clone(), BlockId::latest()).await;
    let mut replacement = old.clone();
    replacement.header.extra_data = vec![1].into();
    replacement.header.hash = replacement.header.hash_slow();
    cache.push_block(replacement, BlockId::latest()).await;
    let old_by_hash = cache
        .get_block(old.header.hash.into(), false)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(old_by_hash.header.hash, old.header.hash);
    let canonical = RpcBlockHash {
        block_hash: old.header.hash,
        require_canonical: Some(true),
    };
    assert!(cache
        .get_block(canonical.into(), false)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn finalized_block_survives_eviction_and_reorg() {
    let cache = BlockCache::<Ethereum>::new();
    let finalized = rpc_block();
    cache
        .push_block(finalized.clone(), BlockId::finalized())
        .await;

    let mut latest = finalized.clone();
    let mut first_child_hash = None;
    for _ in 0..MAX_STATE_HISTORY_LENGTH {
        latest.header.parent_hash = latest.header.hash;
        latest.header.number += 1;
        latest.header.hash = latest.header.hash_slow();
        first_child_hash.get_or_insert(latest.header.hash);
        cache.push_block(latest.clone(), BlockId::latest()).await;
    }

    // Replacing the head clears the canonical index while retaining finalized.
    latest.header.extra_data = vec![1].into();
    latest.header.hash = latest.header.hash_slow();
    cache.push_block(latest, BlockId::latest()).await;

    let canonical_finalized = RpcBlockHash {
        block_hash: finalized.header.hash,
        require_canonical: Some(true),
    };
    for id in [
        BlockId::finalized(),
        finalized.header.number.into(),
        canonical_finalized.into(),
    ] {
        let block = cache.get_block(id, false).await.unwrap().unwrap();
        assert_eq!(block.header.hash, finalized.header.hash);
    }

    // Protecting finalized must still allow other old blocks to be evicted.
    assert!(cache
        .get_block(first_child_hash.unwrap().into(), false)
        .await
        .unwrap()
        .is_none());
}
