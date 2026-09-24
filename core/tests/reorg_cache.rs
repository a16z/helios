use alloy::{
    consensus::{proofs::calculate_transaction_root, proofs::calculate_withdrawals_root},
    rpc::types::{Block, BlockTransactions, Transaction},
};
use helios_common::execution_provider::BlockProvider;
use helios_ethereum::spec::Ethereum;
use helios_test_utils::{rpc_block, rpc_tx};

use helios_core::execution::providers::block::block_cache::BlockCache;

fn full_block() -> Block<Transaction> {
    let mut block = rpc_block();
    let mut tx = rpc_tx();
    block.header.transactions_root = calculate_transaction_root(&[tx.inner.clone()]);
    block.header.withdrawals_root = Some(calculate_withdrawals_root(&[]));
    block.withdrawals = Some(Default::default());
    block.header.hash = block.header.hash_slow();
    block.header.size = None;
    tx.block_hash = Some(block.header.hash);
    tx.block_number = Some(block.header.number);
    tx.transaction_index = Some(0);
    block.transactions = BlockTransactions::Full(vec![tx]);
    block
}

#[tokio::test]
async fn reorg_does_not_resolve_old_hash_to_new_block() {
    let cache = BlockCache::<Ethereum>::new();
    let old = full_block();
    let mut parent = old.clone();
    parent.header.number -= 1;
    parent.header.hash = old.header.parent_hash;
    cache
        .push_block(parent, alloy::eips::BlockId::latest())
        .await;
    cache
        .push_block(old.clone(), alloy::eips::BlockId::latest())
        .await;
    let mut replacement = old.clone();
    replacement.header.extra_data = vec![1].into();
    replacement.header.hash = replacement.header.hash_slow();
    cache
        .push_block(replacement, alloy::eips::BlockId::latest())
        .await;
    let old_by_hash = cache
        .get_block(old.header.hash.into(), false)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(old_by_hash.header.hash, old.header.hash);
    let canonical = alloy::eips::eip1898::RpcBlockHash {
        block_hash: old.header.hash,
        require_canonical: Some(true),
    };
    assert!(cache
        .get_block(canonical.into(), false)
        .await
        .unwrap()
        .is_none());
}
