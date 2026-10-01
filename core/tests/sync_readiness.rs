#[path = "support/metadata.rs"]
mod metadata;

use std::{
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use alloy::{
    eips::BlockId,
    primitives::B256,
    rpc::types::{Block, BlockTransactions},
};
use async_trait::async_trait;
use eyre::{eyre, Result};
use helios_common::execution_provider::BlockProvider;
use helios_core::{
    client::{api::HeliosApi, node::Node},
    consensus::{Consensus, TrustedBlockRef},
    execution::providers::{block::block_cache::BlockCache, rpc::RpcExecutionProvider},
};
use helios_ethereum::spec::Ethereum;
use jsonrpsee::{
    server::{ServerBuilder, ServerHandle},
    types::ErrorObjectOwned,
    RpcModule,
};
use tokio::sync::{mpsc, watch, Notify, Semaphore};

struct TestConsensus {
    latest: Option<mpsc::Receiver<TrustedBlockRef<Block>>>,
    finalized: Option<watch::Receiver<Option<TrustedBlockRef<Block>>>>,
    error: Option<&'static str>,
}

#[async_trait]
impl Consensus<Block> for TestConsensus {
    fn block_recv(&mut self) -> Option<mpsc::Receiver<TrustedBlockRef<Block>>> {
        self.latest.take()
    }
    fn finalized_block_recv(&mut self) -> Option<watch::Receiver<Option<TrustedBlockRef<Block>>>> {
        self.finalized.take()
    }
    fn checkpoint_recv(&self) -> Option<watch::Receiver<Option<B256>>> {
        None
    }
    fn expected_highest_block(&self) -> u64 {
        101
    }
    fn chain_id(&self) -> u64 {
        1
    }
    fn shutdown(&self) -> Result<()> {
        Ok(())
    }
    async fn wait_synced(&self) -> Result<()> {
        match self.error {
            Some(error) => Err(eyre!(error)),
            None => Ok(()),
        }
    }
}

struct GatedBlock {
    block: Block,
    started: Notify,
    release: Semaphore,
}

impl GatedBlock {
    fn new(number: u64, parent: B256) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let (mut block, _) = metadata::fixture(
            vec![metadata::envelopes(false)[0].clone()],
            now,
            11684671,
            0,
            true,
        );
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
        Self {
            block,
            started: Notify::new(),
            release: Semaphore::new(0),
        }
    }
}

struct RpcState {
    latest: GatedBlock,
    finalized: GatedBlock,
    calls: Mutex<Vec<(B256, bool)>>,
    invalid: Option<B256>,
}

type TestNode =
    Node<Ethereum, TestConsensus, RpcExecutionProvider<Ethereum, BlockCache<Ethereum>, ()>>;

struct Harness {
    node: TestNode,
    latest: mpsc::Sender<TrustedBlockRef<Block>>,
    finalized: watch::Sender<Option<TrustedBlockRef<Block>>>,
    rpc: Arc<RpcState>,
    server: ServerHandle,
}

impl Harness {
    async fn new(error: Option<&'static str>, invalid_finalized: Option<bool>) -> Self {
        let finalized = GatedBlock::new(100, B256::ZERO);
        let latest = GatedBlock::new(101, finalized.block.header.hash);
        let invalid = invalid_finalized.map(|finalized_invalid| {
            if finalized_invalid {
                finalized.block.header.hash
            } else {
                latest.block.header.hash
            }
        });
        let rpc = Arc::new(RpcState {
            latest,
            finalized,
            calls: Mutex::default(),
            invalid,
        });
        let server = ServerBuilder::default().build("127.0.0.1:0").await.unwrap();
        let addr = server.local_addr().unwrap();
        let mut module = RpcModule::new(rpc.clone());
        module
            .register_async_method("eth_getBlockByHash", |params, state| async move {
                let (hash, full): (B256, bool) = params.parse().unwrap();
                state.calls.lock().unwrap().push((hash, full));
                let head = if hash == state.latest.block.header.hash {
                    &state.latest
                } else {
                    assert_eq!(hash, state.finalized.block.header.hash);
                    &state.finalized
                };
                head.started.notify_one();
                head.release.acquire().await.unwrap().forget();
                let mut block = head.block.clone();
                if state.invalid == Some(hash) {
                    // Preserve the reported hash while corrupting its authenticated contents.
                    block.header.gas_used += 1;
                }
                Ok::<_, ErrorObjectOwned>(Some(block))
            })
            .unwrap();
        let server = server.start(module);
        let (latest, latest_rx) = mpsc::channel(4);
        let (finalized, finalized_rx) = watch::channel(None);
        let consensus = TestConsensus {
            latest: Some(latest_rx),
            finalized: Some(finalized_rx),
            error,
        };
        let execution = RpcExecutionProvider::<Ethereum, _, ()>::new(
            format!("http://{addr}").parse().unwrap(),
            BlockCache::<Ethereum>::new(),
            metadata::forks(),
        );
        Self {
            node: Node::new(consensus, execution, metadata::forks()),
            latest,
            finalized,
            rpc,
            server,
        }
    }

    async fn publish_hashes(&self) {
        self.latest
            .send(TrustedBlockRef::Hash(self.rpc.latest.block.header.hash))
            .await
            .unwrap();
        self.finalized
            .send(Some(TrustedBlockRef::Hash(
                self.rpc.finalized.block.header.hash,
            )))
            .unwrap();
        // Both requests must start before either response is released: this also
        // catches accidental serialization into two upstream RPC round trips.
        tokio::time::timeout(Duration::from_secs(5), async {
            tokio::join!(
                self.rpc.latest.started.notified(),
                self.rpc.finalized.started.notified()
            );
        })
        .await
        .unwrap();
    }

    fn assert_request_budget(&self) {
        let mut actual = self.rpc.calls.lock().unwrap().clone();
        actual.sort();
        let mut expected = vec![
            (self.rpc.latest.block.header.hash, true),
            (self.rpc.finalized.block.header.hash, true),
        ];
        expected.sort();
        assert_eq!(actual, expected);
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.server.stop().unwrap();
    }
}

async fn wait_until_cached(node: &TestNode, id: BlockId) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while node.execution.get_block(id, false).await.unwrap().is_none() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn waits_for_both_validated_heads_with_two_concurrent_requests() {
    for finalized_first in [false, true] {
        let h = Harness::new(None, None).await;
        h.publish_hashes().await;
        let first_waiter = h.node.wait_synced();
        let second_waiter = h.node.wait_synced();
        tokio::pin!(first_waiter, second_waiter);
        assert!(futures::poll!(&mut first_waiter).is_pending());
        assert!(futures::poll!(&mut second_waiter).is_pending());

        let (first, second, tag) = if finalized_first {
            (&h.rpc.finalized, &h.rpc.latest, BlockId::finalized())
        } else {
            (&h.rpc.latest, &h.rpc.finalized, BlockId::latest())
        };
        first.release.add_permits(1);
        wait_until_cached(&h.node, tag).await;
        assert!(futures::poll!(&mut first_waiter).is_pending());
        assert!(futures::poll!(&mut second_waiter).is_pending());

        second.release.add_permits(1);
        tokio::time::timeout(Duration::from_secs(5), async {
            tokio::try_join!(first_waiter, second_waiter).unwrap();
        })
        .await
        .unwrap();
        assert_eq!(h.node.get_block_number().await.unwrap(), 101);
        assert!(h
            .node
            .get_block(BlockId::finalized(), false)
            .await
            .unwrap()
            .is_some());
        h.node.wait_synced().await.unwrap();
        h.assert_request_budget();
    }
}

#[tokio::test]
async fn invalid_initial_heads_return_errors_without_reporting_ready_or_retrying() {
    for finalized_invalid in [false, true] {
        let h = Harness::new(None, Some(finalized_invalid)).await;
        h.publish_hashes().await;
        h.rpc.latest.release.add_permits(1);
        h.rpc.finalized.release.add_permits(1);
        let error = tokio::time::timeout(Duration::from_secs(5), h.node.wait_synced())
            .await
            .unwrap()
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(if finalized_invalid {
                "initial finalized block"
            } else {
                "initial latest block"
            }),
            "{error}"
        );
        assert!(error.contains("failed local hash validation"), "{error}");
        let tag = if finalized_invalid {
            BlockId::finalized()
        } else {
            BlockId::latest()
        };
        assert!(h
            .node
            .execution
            .get_block(tag, false)
            .await
            .unwrap()
            .is_none());
        h.assert_request_budget();
    }
}

#[tokio::test]
async fn full_latest_block_without_finalized_head_needs_no_rpc_requests() {
    let h = Harness::new(None, None).await;
    let waiter = h.node.wait_synced();
    tokio::pin!(waiter);
    assert!(futures::poll!(&mut waiter).is_pending());
    h.latest
        .send(TrustedBlockRef::Full(h.rpc.latest.block.clone()))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), waiter)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(h.node.get_block_number().await.unwrap(), 101);
    assert!(h.rpc.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn consensus_failure_is_propagated_without_waiting_for_blocks() {
    let h = Harness::new(Some("invalid checkpoint"), None).await;
    let error = tokio::time::timeout(Duration::from_secs(5), h.node.wait_synced())
        .await
        .unwrap()
        .unwrap_err();
    assert_eq!(error.to_string(), "invalid checkpoint");
    assert!(h.rpc.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn stopped_consensus_unblocks_pending_readiness() {
    let mut h = Harness::new(None, None).await;
    // Close the original latest stream without announcing any block.
    let (unused_sender, _) = mpsc::channel(1);
    drop(std::mem::replace(&mut h.latest, unused_sender));
    let error = tokio::time::timeout(Duration::from_secs(5), h.node.wait_synced())
        .await
        .unwrap()
        .unwrap_err();
    assert!(error.to_string().contains("consensus stopped"));
    assert!(h.rpc.calls.lock().unwrap().is_empty());
}
