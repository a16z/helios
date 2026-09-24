use alloy::{
    consensus::{proofs::calculate_transaction_root, proofs::calculate_withdrawals_root},
    primitives::{Address, B256},
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
fn rejects_forged_transaction_sender() {
    let mut block = full_block();
    let BlockTransactions::Full(txs) = &mut block.transactions else {
        panic!()
    };
    let tx = &mut txs[0];
    tx.inner = alloy::consensus::transaction::Recovered::new_unchecked(
        tx.inner.clone().into_inner(),
        Address::ZERO,
    );
    assert!(!Ethereum::validate_block(&mut block));
}
#[test]
fn rejects_forged_cached_transaction_hash_and_location() {
    for (field, value) in [
        ("hash", serde_json::json!(B256::ZERO)),
        ("blockHash", serde_json::json!(B256::ZERO)),
        ("blockNumber", serde_json::json!("0x1")),
        ("transactionIndex", serde_json::json!("0x1")),
    ] {
        let mut block = full_block();
        let BlockTransactions::Full(txs) = &mut block.transactions else {
            panic!()
        };
        let mut json = serde_json::to_value(&txs[0]).unwrap();
        json[field] = value;
        txs[0] = serde_json::from_value(json).unwrap();
        assert!(
            !Ethereum::validate_block(&mut block),
            "accepted forged {field}"
        );
    }
}
#[test]
fn does_not_expose_unproven_optional_block_metadata() {
    let mut block = full_block();
    block.header.size = Some(alloy::primitives::U256::from(123));
    block.header.total_difficulty = Some(alloy::primitives::U256::from(456));
    assert!(Ethereum::validate_block(&mut block));
    assert!(block.header.size.is_none());
    assert!(block.header.total_difficulty.is_none());
    block.uncles.push(B256::ZERO);
    assert!(!Ethereum::validate_block(&mut block));
}
