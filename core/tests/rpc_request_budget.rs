//! Real HTTP RPC + valid account/storage proofs: guard request counts, payloads,
//! and the dependency stages of metadata verification without timing benchmarks.
#[path = "support/metadata.rs"]
mod metadata;

use alloy::{
    consensus::TrieAccount,
    eips::{BlockId, BlockNumberOrTag},
    primitives::{address, keccak256, Address, B256, U256},
    rpc::types::{
        Block, BlockTransactions, EIP1186AccountProofResponse, EIP1186StorageProof, Filter, Log,
        Transaction, TransactionReceipt,
    },
};
use alloy_trie::{proof::ProofRetainer, HashBuilder, Nibbles, KECCAK_EMPTY};
use helios_common::{
    execution_provider::{AccountProvider, BlockProvider, LogProvider, ReceiptProvider},
    network_spec::NetworkSpec,
};
use helios_core::execution::providers::{
    block::block_cache::BlockCache, historical::eip2935::Eip2935Provider, rpc::RpcExecutionProvider,
};
use helios_ethereum::spec::Ethereum;
use jsonrpsee::server::logger::{
    HttpRequest, Logger, MethodKind, Params, SuccessOrError, TransportProtocol,
};
use jsonrpsee::{
    server::{ServerBuilder, ServerHandle},
    types::ErrorObjectOwned,
    RpcModule,
};
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use std::{net::SocketAddr, sync::atomic::Ordering};
use tokio::sync::Barrier;

const HISTORY: Address = address!("0000F90827F1C53a10cb7A02335B175320002935");

struct Fixture {
    block: Block<Transaction>,
    receipts: Vec<TransactionReceipt>,
}
impl Fixture {
    fn new(number: u64, parent: B256, blob: bool) -> Self {
        let txs = metadata::envelopes(false);
        let txs = if blob {
            txs
        } else {
            vec![txs[(number % 2) as usize].clone()]
        };
        let (mut block, mut receipts) = metadata::fixture(txs, 1767747671, 11684671, 2, true);
        block.header.number = number;
        block.header.parent_hash = parent;
        block.header.hash = block.header.hash_slow();
        let BlockTransactions::Full(txs) = &mut block.transactions else {
            panic!()
        };
        for tx in txs {
            tx.block_hash = Some(block.header.hash);
            tx.block_number = Some(number);
        }
        for receipt in &mut receipts {
            receipt.block_hash = Some(block.header.hash);
            receipt.block_number = Some(number);
            for log in &mut receipt
                .inner
                .as_receipt_with_bloom_mut()
                .unwrap()
                .receipt
                .logs
            {
                log.block_hash = receipt.block_hash;
                log.block_number = receipt.block_number;
            }
        }
        Self { block, receipts }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Call {
    method: &'static str,
    params: Value,
    response_bytes: usize,
}
struct State {
    fixtures: Vec<Fixture>,
    latest: Block<Transaction>,
    history_proofs: HashMap<B256, EIP1186AccountProofResponse>,
    logs: Vec<Log>,
    calls: Mutex<Vec<Call>>,
    gates: Option<[Barrier; 3]>,
}
impl State {
    fn record<T: serde::Serialize>(&self, method: &'static str, params: Value, response: &T) {
        self.calls.lock().unwrap().push(Call {
            method,
            params,
            response_bytes: serde_json::to_vec(response).unwrap().len(),
        });
    }
    async fn gate(&self, stage: usize) {
        if let Some(gates) = &self.gates {
            gates[stage].wait().await;
        }
    }
    fn block(&self, id: BlockId) -> &Fixture {
        self.fixtures
            .iter()
            .find(|f| match id {
                BlockId::Hash(h) => f.block.header.hash == h.block_hash,
                BlockId::Number(BlockNumberOrTag::Number(n)) => f.block.header.number == n,
                _ => false,
            })
            .expect("unexpected block request")
    }
}

fn history(fixtures: &[Fixture]) -> (B256, HashMap<B256, EIP1186AccountProofResponse>) {
    let mut entries: Vec<_> = fixtures
        .iter()
        .map(|f| {
            let slot = B256::from(U256::from(f.block.header.number % 8191));
            (
                Nibbles::unpack(keccak256(slot)),
                slot,
                U256::from_be_bytes(f.block.header.hash.0),
            )
        })
        .collect();
    entries.sort_by_key(|e| e.0);
    let mut storage = HashBuilder::default()
        .with_proof_retainer(ProofRetainer::new(entries.iter().map(|e| e.0).collect()));
    for (key, _, value) in &entries {
        storage.add_leaf(*key, &alloy::rlp::encode(value));
    }
    let storage_root = storage.root();
    let nodes = storage.take_proof_nodes();
    let account = TrieAccount {
        nonce: 1,
        balance: U256::ZERO,
        storage_root,
        code_hash: KECCAK_EMPTY,
    };
    let key = Nibbles::unpack(keccak256(HISTORY));
    let mut state = HashBuilder::default().with_proof_retainer(ProofRetainer::new(vec![key]));
    state.add_leaf(key, &alloy::rlp::encode(account));
    let state_root = state.root();
    let account_proof: Vec<_> = state
        .take_proof_nodes()
        .matching_nodes_sorted(&key)
        .into_iter()
        .map(|(_, n)| n)
        .collect();
    let proofs = entries
        .into_iter()
        .map(|(key, slot, value)| {
            (
                slot,
                EIP1186AccountProofResponse {
                    address: HISTORY,
                    balance: U256::ZERO,
                    nonce: 1,
                    code_hash: KECCAK_EMPTY,
                    storage_hash: storage_root,
                    account_proof: account_proof.clone(),
                    storage_proof: vec![EIP1186StorageProof {
                        key: slot.into(),
                        value,
                        proof: nodes
                            .matching_nodes_sorted(&key)
                            .into_iter()
                            .map(|(_, n)| n)
                            .collect(),
                    }],
                },
            )
        })
        .collect();
    (state_root, proofs)
}

struct Harness {
    provider: RpcExecutionProvider<Ethereum, BlockCache<Ethereum>, Eip2935Provider<Ethereum>>,
    state: Arc<State>,
    handle: ServerHandle,
    traffic: Traffic,
}

// Count all HTTP requests and RPC attempts, including unknown methods/retries.
#[derive(Clone, Default)]
struct Traffic {
    requests: Arc<std::sync::atomic::AtomicUsize>,
    calls: Arc<std::sync::atomic::AtomicUsize>,
}
impl Logger for Traffic {
    type Instant = ();
    fn on_connect(&self, _: SocketAddr, _: &HttpRequest, _: TransportProtocol) {}
    fn on_request(&self, _: TransportProtocol) {
        self.requests.fetch_add(1, Ordering::Relaxed);
    }
    fn on_call(&self, _: &str, _: Params, _: MethodKind, _: TransportProtocol) {
        self.calls.fetch_add(1, Ordering::Relaxed);
    }
    fn on_result(&self, _: &str, _: SuccessOrError, _: (), _: TransportProtocol) {}
    fn on_response(&self, _: &str, _: (), _: TransportProtocol) {}
    fn on_disconnect(&self, _: SocketAddr, _: TransportProtocol) {}
}
impl Drop for Harness {
    fn drop(&mut self) {
        let _ = self.handle.stop();
    }
}
impl Harness {
    async fn new(fixtures: Vec<Fixture>, cached: bool, concurrent: bool, empty_logs: bool) -> Self {
        let (root, history_proofs) = history(&fixtures);
        let mut latest = helios_test_utils::rpc_block();
        latest.header.number = fixtures.last().unwrap().block.header.number + 1;
        latest.header.parent_hash = fixtures.last().unwrap().block.header.hash;
        latest.header.state_root = root;
        latest.header.hash = latest.header.hash_slow();
        latest.transactions = BlockTransactions::Full(vec![]);
        let logs = if empty_logs {
            vec![]
        } else {
            fixtures
                .iter()
                .flat_map(|f| f.receipts.iter().flat_map(Ethereum::receipt_logs))
                .collect()
        };
        let state = Arc::new(State {
            fixtures,
            latest,
            history_proofs,
            logs,
            calls: Mutex::new(vec![]),
            gates: concurrent.then(|| std::array::from_fn(|_| Barrier::new(2))),
        });
        let traffic = Traffic::default();
        let server = ServerBuilder::default()
            .set_logger(traffic.clone())
            .build("127.0.0.1:0")
            .await
            .unwrap();
        let url = format!("http://{}", server.local_addr().unwrap());
        let mut rpc = RpcModule::new(state.clone());
        for method in ["eth_getBlockByNumber", "eth_getBlockByHash"] {
            rpc.register_async_method(method, move |params, state| async move {
                let args: Value = params.parse()?;
                assert_eq!(
                    args[1], true,
                    "historical verification must fetch the full body in one request"
                );
                let id = serde_json::from_value(args[0].clone()).unwrap();
                let result = state.block(id).block.clone();
                state.record(method, args, &result);
                state.gate(0).await;
                Ok::<_, ErrorObjectOwned>(result)
            })
            .unwrap();
        }
        rpc.register_async_method("eth_getProof", |params, state| async move {
            let args: Value = params.parse()?;
            let address: Address = serde_json::from_value(args[0].clone()).unwrap();
            let proof = if address == HISTORY {
                let slots: Vec<B256> = serde_json::from_value(args[1].clone()).unwrap();
                assert_eq!(slots.len(), 1);
                let id: BlockId = serde_json::from_value(args[2].clone()).unwrap();
                assert_eq!(id, BlockId::from(state.latest.header.hash));
                state.history_proofs[&slots[0]].clone()
            } else {
                let proof = helios_test_utils::rpc_proof();
                assert_eq!(address, proof.address);
                proof
            };
            state.record("eth_getProof", args, &proof);
            if address == HISTORY {
                state.gate(1).await;
            }
            Ok::<_, ErrorObjectOwned>(proof)
        })
        .unwrap();
        rpc.register_method("eth_getTransactionReceipt", |params, state| {
            let args: Value = params.parse()?;
            let hash: B256 = serde_json::from_value(args[0].clone()).unwrap();
            let receipt = state
                .fixtures
                .iter()
                .flat_map(|f| &f.receipts)
                .find(|r| r.transaction_hash == hash)
                .cloned();
            state.record("eth_getTransactionReceipt", args, &receipt);
            Ok::<_, ErrorObjectOwned>(receipt)
        })
        .unwrap();
        rpc.register_async_method("eth_getBlockReceipts", |params, state| async move {
            let args: Value = params.parse()?;
            let id: BlockId = serde_json::from_value(args[0].clone()).unwrap();
            assert!(
                matches!(id, BlockId::Hash(_)),
                "receipt requests must be pinned to a hash"
            );
            let receipts = state.block(id).receipts.clone();
            state.record("eth_getBlockReceipts", args, &receipts);
            state.gate(2).await;
            Ok::<_, ErrorObjectOwned>(receipts)
        })
        .unwrap();
        rpc.register_method("eth_getLogs", |params, state| {
            let args: Value = params.parse()?;
            state.record("eth_getLogs", args, &state.logs);
            Ok::<_, ErrorObjectOwned>(state.logs.clone())
        })
        .unwrap();
        let handle = server.start(rpc);
        let cache = BlockCache::new();
        if cached {
            for fixture in &state.fixtures {
                cache
                    .push_block(fixture.block.clone(), BlockId::latest())
                    .await;
            }
        }
        cache
            .push_block(state.latest.clone(), BlockId::latest())
            .await;
        let provider = RpcExecutionProvider::with_historical_provider(
            url.parse().unwrap(),
            cache,
            Eip2935Provider::new(),
            metadata::forks(),
        );
        Self {
            provider,
            state,
            handle,
            traffic,
        }
    }
    fn take_calls(&self, expected: &[&str]) -> Vec<Call> {
        assert_eq!(
            self.traffic.requests.swap(0, Ordering::Relaxed),
            expected.len(),
            "HTTP request count changed"
        );
        assert_eq!(
            self.traffic.calls.swap(0, Ordering::Relaxed),
            expected.len(),
            "RPC request count changed"
        );
        let calls = std::mem::take(&mut *self.state.calls.lock().unwrap());
        assert_eq!(calls.iter().map(|c| c.method).collect::<Vec<_>>(), expected);
        println!(
            "RPC_BUDGET methods={} requests={} params_bytes={} response_payload_bytes={}",
            expected.join(","),
            calls.len(),
            calls
                .iter()
                .map(|c| serde_json::to_vec(&c.params).unwrap().len())
                .sum::<usize>(),
            calls.iter().map(|c| c.response_bytes).sum::<usize>()
        );
        calls
    }
}

#[tokio::test]
async fn cached_blocks_accounts_and_receipts_keep_their_request_budgets() {
    let h = Harness::new(
        vec![Fixture::new(100, B256::ZERO, true)],
        true,
        false,
        false,
    )
    .await;
    for full in [false, true] {
        h.provider
            .get_block(BlockId::number(100), full)
            .await
            .unwrap()
            .unwrap();
        h.take_calls(&[]);
    }
    let account = helios_test_utils::rpc_proof().address;
    h.provider
        .get_account(account, &[], false, BlockId::number(100))
        .await
        .unwrap();
    h.take_calls(&["eth_getProof"]);
    h.provider
        .get_block_receipts(BlockId::number(100))
        .await
        .unwrap()
        .unwrap();
    h.take_calls(&["eth_getBlockReceipts"]);
    // An ordinary transaction sharing a BPO2 block with a blob transaction.
    let expected = &h.state.fixtures[0].receipts[0];
    assert_eq!(
        h.provider
            .get_receipt(expected.transaction_hash)
            .await
            .unwrap()
            .as_ref(),
        Some(expected)
    );
    h.take_calls(&["eth_getTransactionReceipt", "eth_getBlockReceipts"]);
}

#[tokio::test]
async fn historical_reads_keep_the_same_full_body_payload_and_request_budgets() {
    let h = Harness::new(
        vec![Fixture::new(100, B256::ZERO, true)],
        false,
        false,
        false,
    )
    .await;
    let mut traces = vec![];
    for full in [false, true] {
        let block = h
            .provider
            .get_block(BlockId::number(100), full)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(block.transactions.is_full(), full);
        traces.push(h.take_calls(&["eth_getBlockByNumber", "eth_getProof"]));
    }
    assert_eq!(
        traces[0], traces[1],
        "hash/full output modes must use identical upstream requests and response payloads"
    );
    h.provider
        .get_account(
            helios_test_utils::rpc_proof().address,
            &[],
            false,
            BlockId::number(100),
        )
        .await
        .unwrap();
    h.take_calls(&["eth_getBlockByNumber", "eth_getProof", "eth_getProof"]);
    h.provider
        .get_block_receipts(BlockId::number(100))
        .await
        .unwrap()
        .unwrap();
    h.take_calls(&[
        "eth_getBlockByNumber",
        "eth_getProof",
        "eth_getBlockReceipts",
    ]);
    let expected = &h.state.fixtures[0].receipts[0];
    assert_eq!(
        h.provider
            .get_receipt(expected.transaction_hash)
            .await
            .unwrap()
            .as_ref(),
        Some(expected)
    );
    h.take_calls(&[
        "eth_getTransactionReceipt",
        "eth_getBlockByHash",
        "eth_getProof",
        "eth_getBlockReceipts",
    ]);
}

#[tokio::test]
async fn multiblock_log_queries_preserve_parallel_rpc_stages() {
    for cached in [true, false] {
        let first = Fixture::new(100, B256::ZERO, false);
        let second = Fixture::new(101, first.block.header.hash, false);
        let h = Harness::new(vec![first, second], cached, true, false).await;
        // Every remote stage waits for both blocks. Serializing any stage times out.
        let result = tokio::time::timeout(
            Duration::from_secs(5),
            h.provider
                .get_logs(&Filter::new().from_block(1).to_block(102)),
        )
        .await
        .expect("block verification RPCs must run concurrently")
        .unwrap();
        assert_eq!(result, h.state.logs);
        let expected: &[&str] = if cached {
            &[
                "eth_getLogs",
                "eth_getBlockReceipts",
                "eth_getBlockReceipts",
            ]
        } else {
            &[
                "eth_getLogs",
                "eth_getBlockByNumber",
                "eth_getBlockByNumber",
                "eth_getProof",
                "eth_getProof",
                "eth_getBlockReceipts",
                "eth_getBlockReceipts",
            ]
        };
        h.take_calls(expected);
    }
}

#[tokio::test]
async fn empty_log_results_only_make_the_original_log_request() {
    for cached in [true, false] {
        let h = Harness::new(
            vec![Fixture::new(100, B256::ZERO, false)],
            cached,
            false,
            true,
        )
        .await;
        assert!(h
            .provider
            .get_logs(&Filter::new().from_block(1).to_block(101))
            .await
            .unwrap()
            .is_empty());
        h.take_calls(&["eth_getLogs"]);
    }
}
