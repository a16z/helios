use std::sync::{Arc, Mutex};

use alloy::{
    eips::{eip1898::RpcBlockHash, BlockId},
    primitives::{address, bytes, B256, U256},
    rpc::types::{
        state::{AccountOverride, StateOverride},
        Block, EIP1186AccountProofResponse, EIP1186StorageProof, Transaction, TransactionRequest,
    },
};
use alloy_trie::{EMPTY_ROOT_HASH, KECCAK_EMPTY};
use helios_common::{
    execution_provider::{AccountProvider, BlockProvider},
    fork_schedule::ForkSchedule,
    network_spec::NetworkSpec,
    types::EvmError,
};
use helios_core::execution::{
    cache::CachingProvider,
    constants::MAX_STATE_HISTORY_LENGTH,
    providers::{block::block_cache::BlockCache, rpc::RpcExecutionProvider},
};
use helios_ethereum::spec::Ethereum;
use helios_test_utils::rpc_block;
use jsonrpsee::{server::ServerBuilder, types::ErrorObjectOwned, RpcModule};
use tokio::sync::Notify;

fn block() -> Block<Transaction> {
    let mut block = rpc_block();
    block.header.state_root = EMPTY_ROOT_HASH;
    block.header.hash = block.header.hash_slow();
    block
}

fn child(parent: &Block<Transaction>) -> Block<Transaction> {
    let mut block = parent.clone();
    block.header.number += 1;
    block.header.parent_hash = parent.header.hash;
    block.header.hash = block.header.hash_slow();
    block
}

#[tokio::test]
async fn replacements_purge_old_hashes_and_advance_generation() {
    for tag in [BlockId::latest(), BlockId::finalized()] {
        let cache = BlockCache::<Ethereum>::new();
        let parent = block();
        let head = child(&parent);
        cache.push_block(parent, BlockId::latest()).await;
        cache.push_block(head.clone(), tag).await;
        cache.push_block(head.clone(), BlockId::latest()).await;
        assert_eq!(cache.reorg_generation().await, 0);

        let mut replacement = head.clone();
        replacement.header.extra_data = vec![1].into();
        replacement.header.hash = replacement.header.hash_slow();
        cache.push_block(replacement.clone(), tag).await;
        assert_eq!(cache.reorg_generation().await, 1);
        for require_canonical in [None, Some(false), Some(true)] {
            let id = RpcBlockHash {
                block_hash: head.header.hash,
                require_canonical,
            };
            assert!(cache.get_block(id.into(), false).await.unwrap().is_none());
        }
        assert_eq!(
            cache
                .get_block(head.header.number.into(), false)
                .await
                .unwrap()
                .unwrap()
                .header
                .hash,
            replacement.header.hash
        );
    }
}

#[tokio::test]
async fn finality_survives_eviction_and_history_reset() {
    let cache = BlockCache::<Ethereum>::new();
    let finalized = block();
    cache
        .push_block(finalized.clone(), BlockId::finalized())
        .await;
    let first_child = child(&finalized);
    let mut head = finalized.clone();
    for _ in 0..MAX_STATE_HISTORY_LENGTH {
        head = child(&head);
        cache.push_block(head.clone(), BlockId::latest()).await;
    }
    assert_eq!(cache.reorg_generation().await, 0);
    assert!(cache
        .get_block(first_child.header.hash.into(), false)
        .await
        .unwrap()
        .is_none());

    // A gap is conservatively treated as a history reset, too.
    let next = child(&child(&head));
    cache.push_block(next.clone(), BlockId::latest()).await;
    cache.push_block(next, BlockId::latest()).await;
    assert_eq!(cache.reorg_generation().await, 1);
    assert!(cache
        .get_block(head.header.hash.into(), false)
        .await
        .unwrap()
        .is_none());
    for id in [
        BlockId::finalized(),
        finalized.header.number.into(),
        finalized.header.hash.into(),
    ] {
        assert_eq!(
            cache
                .get_block(id, false)
                .await
                .unwrap()
                .unwrap()
                .header
                .hash,
            finalized.header.hash
        );
    }
}

// Valid absence proofs against an empty state trie keep the execution tests local.
// Optionally pause the first proof after the provider has selected its block.
async fn rpc_server(
    pause: Option<Arc<(Notify, Notify)>>,
) -> (url::Url, jsonrpsee::server::ServerHandle) {
    let server = ServerBuilder::default().build("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", server.local_addr().unwrap())
        .parse()
        .unwrap();
    let mut methods = RpcModule::new(Mutex::new(pause));
    methods
        .register_async_method("eth_getProof", |params, context| async move {
            let (address, slots, _): (_, Vec<B256>, BlockId) = params.parse()?;
            let pause = context.lock().unwrap().take();
            if let Some(pause) = pause {
                pause.0.notify_one();
                pause.1.notified().await;
            }
            Ok::<_, ErrorObjectOwned>(EIP1186AccountProofResponse {
                address,
                balance: U256::ZERO,
                nonce: 0,
                code_hash: KECCAK_EMPTY,
                storage_hash: EMPTY_ROOT_HASH,
                account_proof: vec![],
                storage_proof: slots
                    .into_iter()
                    .map(|key| EIP1186StorageProof {
                        key: key.into(),
                        value: U256::ZERO,
                        proof: vec![],
                    })
                    .collect(),
            })
        })
        .unwrap();
    methods
        .register_method("eth_createAccessList", |_, _| {
            Ok::<_, ErrorObjectOwned>(serde_json::json!({"accessList": [], "gasUsed": "0x0"}))
        })
        .unwrap();
    (url, server.start(methods))
}

#[tokio::test]
async fn orphaned_accounts_are_rejected_even_on_cache_hits() {
    let (url, server) = rpc_server(None).await;
    let provider = CachingProvider::new(RpcExecutionProvider::<Ethereum, _, ()>::new(
        url,
        BlockCache::new(),
    ));
    let parent = block();
    let head = child(&parent);
    provider.push_block(parent, BlockId::latest()).await;
    provider.push_block(head.clone(), BlockId::latest()).await;
    let address = address!("1111111111111111111111111111111111111111");
    provider
        .get_account(address, &[], true, head.header.hash.into())
        .await
        .unwrap();

    let mut replacement = head.clone();
    replacement.header.extra_data = vec![1].into();
    replacement.header.hash = replacement.header.hash_slow();
    provider.push_block(replacement, BlockId::latest()).await;
    for require_canonical in [None, Some(false), Some(true)] {
        let id = RpcBlockHash {
            block_hash: head.header.hash,
            require_canonical,
        };
        assert!(provider
            .get_account(address, &[], true, id.into())
            .await
            .is_err());
    }
    server.stop().unwrap();
}

fn network_block<N: NetworkSpec>(block: &Block<Transaction>) -> N::BlockResponse {
    serde_json::from_value(serde_json::to_value(block).unwrap()).unwrap()
}

async fn check_execution_guard<N: NetworkSpec>() {
    // Include returning to the original fork: checking only the final hash misses this.
    for (reorg, restore_original) in [(false, false), (true, false), (true, true)] {
        let pause = Arc::new((Notify::new(), Notify::new()));
        let (url, server) = rpc_server(Some(pause.clone())).await;
        let provider = Arc::new(CachingProvider::new(RpcExecutionProvider::<N, _, ()>::new(
            url,
            BlockCache::<N>::new(),
        )));
        let caller = address!("1111111111111111111111111111111111111111");
        let mut parent = block();
        parent.header.beneficiary = caller;
        parent.header.hash = parent.header.hash_slow();
        let head = child(&parent);
        provider
            .push_block(network_block::<N>(&parent), BlockId::latest())
            .await;
        provider
            .push_block(network_block::<N>(&head), BlockId::latest())
            .await;

        // Return BLOCKHASH(NUMBER - 1) after the head changes during proof fetching.
        let overrides = StateOverride::from_iter([(
            caller,
            AccountOverride {
                balance: Some(U256::from(1_000_000_000u64)),
                code: Some(bytes!("600143034060005260206000f3")),
                ..Default::default()
            },
        )]);
        let tx = TransactionRequest::default()
            .from(caller)
            .to(caller)
            .gas_limit(100_000)
            .gas_price(0);
        let tx = serde_json::from_value(serde_json::to_value(tx).unwrap()).unwrap();
        let forks = ForkSchedule {
            london_timestamp: 0,
            bedrock_timestamp: 0,
            ..Default::default()
        };
        let execution = N::transact(
            &tx,
            false,
            provider.clone(),
            1,
            forks,
            head.header.hash.into(),
            Some(overrides),
        );
        let update_chain = async {
            pause.0.notified().await;
            if reorg {
                let mut replacement = parent.clone();
                replacement.header.extra_data = vec![1].into();
                replacement.header.hash = replacement.header.hash_slow();
                provider
                    .push_block(network_block::<N>(&replacement), BlockId::latest())
                    .await;
                provider
                    .push_block(network_block::<N>(&child(&replacement)), BlockId::latest())
                    .await;
                if restore_original {
                    provider
                        .push_block(network_block::<N>(&parent), BlockId::latest())
                        .await;
                    provider
                        .push_block(network_block::<N>(&head), BlockId::latest())
                        .await;
                }
            } else {
                provider
                    .push_block(network_block::<N>(&child(&head)), BlockId::latest())
                    .await;
            }
            pause.1.notify_one();
        };
        let (result, ()) = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            tokio::join!(execution, update_chain)
        })
        .await
        .expect("execution did not complete");
        if reorg {
            assert!(matches!(result, Err(EvmError::Reorg)), "{result:?}");
        } else {
            assert_eq!(
                result.unwrap().0.output().unwrap().as_ref(),
                parent.header.hash.as_slice()
            );
        }
        server.stop().unwrap();
    }
}

#[tokio::test]
async fn ethereum_execution_rejects_reorgs() {
    check_execution_guard::<Ethereum>().await;
}

#[tokio::test]
async fn opstack_execution_rejects_reorgs() {
    check_execution_guard::<helios_opstack::spec::OpStack>().await;
}
