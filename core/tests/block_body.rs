use alloy::{
    consensus::{proofs::calculate_transaction_root, proofs::calculate_withdrawals_root},
    primitives::B256,
    rpc::types::{Block, BlockTransactions, Transaction},
};
use helios_common::network_spec::NetworkSpec;
use helios_ethereum::spec::Ethereum;
use helios_test_utils::{rpc_block, rpc_tx};

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

#[test]
fn rejects_unproven_transaction_hashes() {
    let mut block = full_block();
    assert!(Ethereum::is_hash_valid(&block));
    block.transactions = BlockTransactions::Hashes(vec![B256::repeat_byte(1)]);
    assert!(!Ethereum::is_hash_valid(&block));
}
#[test]
fn rejects_missing_withdrawals() {
    let mut block = full_block();
    assert!(Ethereum::is_hash_valid(&block));
    block.withdrawals = None;
    assert!(!Ethereum::is_hash_valid(&block));
}
