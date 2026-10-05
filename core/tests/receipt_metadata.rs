use alloy::{
    consensus::{
        proofs::calculate_transaction_root, proofs::calculate_withdrawals_root, TxReceipt,
    },
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

fn block_with_receipt() -> (Block<Transaction>, alloy::rpc::types::TransactionReceipt) {
    use alloy::{consensus::Transaction as _, network::TransactionResponse};
    let mut block = full_block();
    let mut receipt = helios_test_utils::rpc_tx_receipt();
    receipt
        .inner
        .as_receipt_with_bloom_mut()
        .unwrap()
        .receipt
        .cumulative_gas_used = receipt.gas_used;
    block.header.receipts_root =
        helios_core::execution::proof::ordered_trie_root_noop_encoder(&[Ethereum::encode_receipt(
            &receipt,
        )]);
    block.header.logs_bloom = receipt.inner.bloom();
    block.header.hash = block.header.hash_slow();
    let BlockTransactions::Full(txs) = &mut block.transactions else {
        panic!()
    };
    let tx = &mut txs[0];
    tx.block_hash = Some(block.header.hash);
    receipt.transaction_hash = tx.tx_hash();
    receipt.transaction_index = Some(0);
    receipt.block_hash = Some(block.header.hash);
    receipt.block_number = Some(block.header.number);
    receipt.from = tx.from();
    receipt.to = tx.to();
    receipt.contract_address = None;
    receipt.effective_gas_price = tx.effective_gas_price(block.header.base_fee_per_gas);
    receipt.blob_gas_used = None;
    receipt.blob_gas_price = None;
    for (i, log) in receipt
        .inner
        .as_receipt_with_bloom_mut()
        .unwrap()
        .receipt
        .logs
        .iter_mut()
        .enumerate()
    {
        log.block_hash = receipt.block_hash;
        log.block_number = receipt.block_number;
        log.block_timestamp = Some(block.header.timestamp);
        log.transaction_hash = Some(receipt.transaction_hash);
        log.transaction_index = Some(0);
        log.log_index = Some(i as u64);
        log.removed = false;
    }
    (block, receipt)
}
#[test]
fn rejects_forged_receipt_metadata_even_with_valid_receipts_root() {
    use helios_core::execution::proof::verify_authenticated_block_receipts as verify_block_receipts;
    use serde_json::json;
    let (block, receipt) = block_with_receipt();
    let forks = Default::default();
    verify_block_receipts::<Ethereum>(std::slice::from_ref(&receipt), &block, &forks).unwrap();
    for (field, value) in [
        ("transactionHash", json!(B256::ZERO)),
        ("transactionIndex", json!("0x1")),
        ("blockHash", json!(B256::ZERO)),
        ("blockNumber", json!("0x1")),
        ("from", json!(Address::ZERO)),
        ("to", json!(Address::ZERO)),
        ("contractAddress", json!(Address::ZERO)),
        ("gasUsed", json!("0x1")),
        ("effectiveGasPrice", json!("0x1")),
        ("blobGasUsed", json!("0x1")),
        ("blobGasPrice", json!("0x1")),
    ] {
        let mut json = serde_json::to_value(&receipt).unwrap();
        json[field] = value;
        let forged = serde_json::from_value(json).unwrap();
        assert_eq!(
            Ethereum::encode_receipt(&receipt),
            Ethereum::encode_receipt(&forged)
        );
        assert!(
            verify_block_receipts::<Ethereum>(&[forged], &block, &forks).is_err(),
            "accepted forged {field}"
        );
    }
}
#[test]
fn rejects_forged_log_locations_even_with_valid_receipts_root() {
    use helios_core::execution::proof::verify_authenticated_block_receipts as verify_block_receipts;
    use serde_json::json;
    let (block, receipt) = block_with_receipt();
    for (field, value) in [
        ("transactionHash", json!(B256::ZERO)),
        ("transactionIndex", json!("0x1")),
        ("blockHash", json!(B256::ZERO)),
        ("blockNumber", json!("0x1")),
        ("blockTimestamp", json!("0x1")),
        ("logIndex", json!("0x1")),
        ("removed", json!(true)),
    ] {
        let mut json = serde_json::to_value(&receipt).unwrap();
        json["logs"][0][field] = value;
        let forged = serde_json::from_value(json).unwrap();
        assert_eq!(
            Ethereum::encode_receipt(&receipt),
            Ethereum::encode_receipt(&forged)
        );
        assert!(
            verify_block_receipts::<Ethereum>(&[forged], &block, &Default::default()).is_err(),
            "accepted forged log {field}"
        );
    }
}
