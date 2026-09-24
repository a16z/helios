use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use alloy::{
    consensus::{
        proofs::{calculate_transaction_root, calculate_withdrawals_root},
        Transaction as _, TxReceipt,
    },
    eips::BlockId,
    network::TransactionResponse,
    primitives::{Bytes, B256},
    rpc::types::{Block, BlockTransactions, Filter, Log, Transaction, TransactionReceipt},
};
use eyre::Result;
use helios_common::{
    execution_provider::{BlockProvider, LogProvider},
    network_spec::NetworkSpec,
};
use helios_core::execution::{
    proof::{ordered_trie_root_noop_encoder, verify_authenticated_block_receipts},
    providers::{block::block_cache::BlockCache, rpc::RpcExecutionProvider},
};
use helios_ethereum::spec::Ethereum;
use helios_test_utils::{rpc_block, rpc_tx, rpc_tx_receipt};
use jsonrpsee::{server::ServerBuilder, types::ErrorObjectOwned, RpcModule};
use serde_json::json;

struct Fixture {
    block: Block<Transaction>,
    receipts: Vec<TransactionReceipt>,
}

impl Fixture {
    fn new(number: u64, receipt_timestamp: bool) -> Self {
        Self::with_parent(number, receipt_timestamp, B256::ZERO)
    }

    fn with_parent(number: u64, receipt_timestamp: bool, parent_hash: B256) -> Self {
        let mut block = rpc_block();
        let mut tx = rpc_tx();
        let mut receipt = rpc_tx_receipt();
        let inner = &mut receipt.inner.as_receipt_with_bloom_mut().unwrap().receipt;
        inner.cumulative_gas_used = receipt.gas_used;
        // Distinct payloads at two valid indices catch matching the wrong log
        // within a receipt, even when its transaction hash is correct.
        let mut second_log = inner.logs[0].clone();
        second_log.inner.data.data = Bytes::from_static(b"second log");
        inner.logs.push(second_log);

        block.header.number = number;
        block.header.parent_hash = parent_hash;
        block.header.gas_used = receipt.gas_used;
        block.header.transactions_root = calculate_transaction_root(&[tx.inner.clone()]);
        block.header.withdrawals_root = Some(calculate_withdrawals_root(&[]));
        block.withdrawals = Some(Default::default());
        block.header.receipts_root =
            ordered_trie_root_noop_encoder(&[Ethereum::encode_receipt(&receipt)]);
        block.header.logs_bloom = receipt.inner.bloom();
        block.header.hash = block.header.hash_slow();
        block.header.size = None;

        tx.block_hash = Some(block.header.hash);
        tx.block_number = Some(number);
        tx.transaction_index = Some(0);
        receipt.transaction_hash = tx.tx_hash();
        receipt.transaction_index = Some(0);
        receipt.block_hash = Some(block.header.hash);
        receipt.block_number = Some(number);
        receipt.from = tx.from();
        receipt.to = tx.to();
        receipt.contract_address = None;
        receipt.effective_gas_price = tx.effective_gas_price(block.header.base_fee_per_gas);
        receipt.blob_gas_used = None;
        receipt.blob_gas_price = None;
        for (index, log) in receipt
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
            log.block_timestamp = receipt_timestamp.then_some(block.header.timestamp);
            log.transaction_hash = Some(receipt.transaction_hash);
            log.transaction_index = receipt.transaction_index;
            log.log_index = Some(index as u64);
            log.removed = false;
        }
        block.transactions = BlockTransactions::Full(vec![tx]);
        let receipts = vec![receipt];
        assert_eq!(block.header.hash, block.header.hash_slow());
        verify_authenticated_block_receipts::<Ethereum>(&receipts, &block, &Default::default())
            .unwrap();
        Self { block, receipts }
    }

    fn logs(&self) -> Vec<Log> {
        self.receipts
            .iter()
            .flat_map(Ethereum::receipt_logs)
            .collect()
    }
}

struct RpcState {
    receipts: HashMap<B256, Vec<TransactionReceipt>>,
    logs: Vec<Log>,
    receipt_requests: Mutex<Vec<B256>>,
}

async fn get_logs(
    fixtures: &[Fixture],
    logs: Vec<Log>,
    filter: Filter,
) -> (Result<Vec<Log>>, Vec<B256>) {
    let server = ServerBuilder::default().build("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", server.local_addr().unwrap());
    let state = Arc::new(RpcState {
        receipts: fixtures
            .iter()
            .map(|f| (f.block.header.hash, f.receipts.clone()))
            .collect(),
        logs,
        receipt_requests: Mutex::new(Vec::new()),
    });
    let mut methods = RpcModule::new(state.clone());
    methods
        .register_method("eth_getLogs", |_, state| {
            Ok::<_, ErrorObjectOwned>(state.logs.clone())
        })
        .unwrap();
    methods
        .register_method("eth_getBlockReceipts", |params, state| {
            let BlockId::Hash(hash) = params.one::<BlockId>()? else {
                panic!("receipts must be requested by authenticated block hash");
            };
            let hash = B256::from(hash);
            state.receipt_requests.lock().unwrap().push(hash);
            Ok::<_, ErrorObjectOwned>(state.receipts.get(&hash).cloned())
        })
        .unwrap();
    let handle = server.start(methods);
    let cache = BlockCache::<Ethereum>::new();
    for fixture in fixtures {
        cache
            .push_block(fixture.block.clone(), BlockId::latest())
            .await;
    }
    let provider = RpcExecutionProvider::<Ethereum, _, ()>::new(
        url.parse().unwrap(),
        cache,
        Default::default(),
    );
    let result = provider.get_logs(&filter).await;
    handle.stop().unwrap();
    handle.stopped().await;
    let requests = state.receipt_requests.lock().unwrap().clone();
    (result, requests)
}

#[tokio::test]
async fn rejects_forged_log_metadata_with_genuine_receipts() {
    let fixture = Fixture::new(100, true);
    let original = fixture.logs();
    for (field, value) in [
        ("transactionIndex", json!("0x22b8")),
        ("logIndex", json!("0x270f")),
        ("logIndex", json!("0x1")),
        ("removed", json!(true)),
        ("blockTimestamp", json!("0x1")),
        ("transactionHash", json!(B256::ZERO)),
        ("blockHash", json!(B256::ZERO)),
    ] {
        let mut forged = serde_json::to_value(&original[0]).unwrap();
        forged[field] = value;
        let forged: Log = serde_json::from_value(forged).unwrap();
        assert_eq!(forged.inner, original[0].inner);
        let (result, requests) = get_logs(
            std::slice::from_ref(&fixture),
            vec![forged],
            Filter::new().from_block(100).to_block(100),
        )
        .await;
        assert!(result.is_err(), "accepted forged {field}");
        assert_eq!(requests, vec![fixture.block.header.hash]);
    }
}

#[tokio::test]
async fn rejects_missing_log_metadata_without_panicking() {
    let fixture = Fixture::new(100, true);
    for field in [
        "blockNumber",
        "blockHash",
        "transactionHash",
        "transactionIndex",
        "logIndex",
    ] {
        let mut forged = serde_json::to_value(&fixture.logs()[0]).unwrap();
        forged[field] = serde_json::Value::Null;
        let (result, _) = get_logs(
            std::slice::from_ref(&fixture),
            vec![serde_json::from_value(forged).unwrap()],
            Filter::new().at_block_hash(fixture.block.header.hash),
        )
        .await;
        assert!(result.is_err(), "accepted missing {field}");
    }
}

#[tokio::test]
async fn rejects_unverified_block_numbers() {
    let fixture = Fixture::new(100, true);
    for number in [99, 101] {
        let mut forged = fixture.logs()[0].clone();
        forged.block_number = Some(number);
        let (result, requests) = get_logs(
            std::slice::from_ref(&fixture),
            vec![forged],
            Filter::new().from_block(1).to_block(101),
        )
        .await;
        assert!(result.is_err(), "accepted unverified block {number}");
        assert!(requests.is_empty());
    }
}

#[tokio::test]
async fn rejects_forged_block_number_alongside_genuine_logs() {
    let fixture = Fixture::new(100, true);
    let mut logs = fixture.logs();
    // The genuine log still causes this transaction's receipts to be fetched.
    // Its other log must not be accepted as belonging to an unverified block.
    logs[1].block_number = Some(99);
    let (result, requests) = get_logs(
        std::slice::from_ref(&fixture),
        logs,
        Filter::new().from_block(1).to_block(100),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(requests, vec![fixture.block.header.hash]);
}

#[tokio::test]
async fn accepts_valid_logs_with_optional_timestamps() {
    for receipt_timestamp in [false, true] {
        let fixture = Fixture::new(100, receipt_timestamp);
        for log_timestamp in [false, true] {
            let mut logs = fixture.logs();
            for log in &mut logs {
                log.block_timestamp = log_timestamp.then_some(fixture.block.header.timestamp);
            }
            let (result, requests) = get_logs(
                std::slice::from_ref(&fixture),
                logs.clone(),
                Filter::new().at_block_hash(fixture.block.header.hash),
            )
            .await;
            assert_eq!(result.unwrap(), logs);
            assert_eq!(requests, vec![fixture.block.header.hash]);
        }
    }
}

#[tokio::test]
async fn rejects_forged_timestamp_when_receipt_omits_it() {
    let fixture = Fixture::new(100, false);
    let mut forged = fixture.logs()[0].clone();
    forged.block_timestamp = Some(fixture.block.header.timestamp + 1);
    let (result, _) = get_logs(
        std::slice::from_ref(&fixture),
        vec![forged],
        Filter::new().at_block_hash(fixture.block.header.hash),
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn rejects_payload_from_a_different_log_index() {
    let fixture = Fixture::new(100, true);
    let logs = fixture.logs();
    let mut forged = logs[0].clone();
    forged.inner = logs[1].inner.clone();
    let (result, _) = get_logs(
        std::slice::from_ref(&fixture),
        vec![forged],
        Filter::new().at_block_hash(fixture.block.header.hash),
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn verifies_logs_from_multiple_blocks_with_one_receipt_request_each() {
    let first = Fixture::new(100, true);
    let second = Fixture::with_parent(101, true, first.block.header.hash);
    let fixtures = [first, second];
    let logs = fixtures.iter().flat_map(Fixture::logs).collect::<Vec<_>>();
    let (result, mut requests) = get_logs(
        &fixtures,
        logs.clone(),
        Filter::new().from_block(1).to_block(101),
    )
    .await;
    assert_eq!(result.unwrap(), logs);
    requests.sort();
    let mut expected = fixtures
        .iter()
        .map(|f| f.block.header.hash)
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(requests, expected);
}

#[tokio::test]
async fn empty_results_do_not_fetch_receipts() {
    let fixture = Fixture::new(100, true);
    let (result, requests) = get_logs(
        &[fixture],
        vec![],
        Filter::new().from_block(1).to_block(100),
    )
    .await;
    assert!(result.unwrap().is_empty());
    assert!(requests.is_empty());
}
