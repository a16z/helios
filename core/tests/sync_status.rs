#[path = "support/metadata.rs"]
mod metadata;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use alloy::{
    primitives::{B256, U256},
    rpc::types::{Block, SyncStatus},
};
use async_trait::async_trait;
use eyre::Result;
use helios_core::{
    client::{api::HeliosApi, node::Node},
    consensus::{Consensus, TrustedBlockRef},
    execution::providers::{block::block_cache::BlockCache, rpc::RpcExecutionProvider},
};
use helios_ethereum::spec::Ethereum;
use tokio::sync::{mpsc, watch};

const STALE_HEAD_AGE: u64 = 120;

struct TestConsensus {
    latest: Option<mpsc::Receiver<TrustedBlockRef<Block>>>,
    finalized: Option<watch::Receiver<Option<TrustedBlockRef<Block>>>>,
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
        12_346
    }
    fn chain_id(&self) -> u64 {
        1
    }
    fn shutdown(&self) -> Result<()> {
        Ok(())
    }
    async fn wait_synced(&self) -> Result<()> {
        Ok(())
    }
}

type TestNode =
    Node<Ethereum, TestConsensus, RpcExecutionProvider<Ethereum, BlockCache<Ethereum>, ()>>;

struct Harness {
    node: TestNode,
    // Kept alive so the background node task is not torn down mid-test.
    _latest: mpsc::Sender<TrustedBlockRef<Block>>,
    _finalized: watch::Sender<Option<TrustedBlockRef<Block>>>,
}

impl Harness {
    /// Builds a node whose latest head is block `number`, timestamped `age`
    /// seconds in the past. The block is supplied whole, so no RPC round trip
    /// is needed to cache it.
    async fn new(number: u64, age: u64) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - age;
        let (mut block, _) = metadata::fixture(
            vec![metadata::envelopes(false)[0].clone()],
            timestamp,
            11684671,
            0,
            true,
        );
        block.header.number = number;
        block.header.hash = block.header.hash_slow();

        let (latest, latest_rx) = mpsc::channel(4);
        let (finalized, finalized_rx) = watch::channel(None);
        let consensus = TestConsensus {
            latest: Some(latest_rx),
            finalized: Some(finalized_rx),
        };
        let execution = RpcExecutionProvider::<Ethereum, _, ()>::new(
            "http://127.0.0.1:0".parse().unwrap(),
            BlockCache::<Ethereum>::new(),
            metadata::forks(),
        );
        let node = Node::new(consensus, execution, metadata::forks());
        let harness = Self {
            node,
            _latest: latest,
            _finalized: finalized,
        };
        harness
            ._latest
            .send(TrustedBlockRef::Full(block))
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), harness.node.wait_synced())
            .await
            .unwrap()
            .unwrap();
        harness
    }
}

#[tokio::test]
async fn stale_head_reports_the_block_it_synced_to() {
    let h = Harness::new(12_345, STALE_HEAD_AGE).await;

    match h.node.syncing().await.unwrap() {
        SyncStatus::Info(info) => {
            assert_eq!(info.current_block, U256::from(12_345));
            assert_eq!(info.highest_block, U256::from(12_346));
        }
        status => panic!("expected syncing info for a stale head, got {status:?}"),
    }

    // Out of sync also means the freshness-gated accessor still fails.
    assert!(h.node.get_block_number().await.is_err());
}

#[tokio::test]
async fn fresh_head_reports_not_syncing() {
    let h = Harness::new(12_345, 0).await;

    assert!(matches!(h.node.syncing().await.unwrap(), SyncStatus::None));
    assert_eq!(h.node.get_block_number().await.unwrap(), U256::from(12_345));
}
