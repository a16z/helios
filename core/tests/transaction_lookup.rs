use alloy::{
    consensus::{proofs::calculate_transaction_root, proofs::calculate_withdrawals_root},
    primitives::B256,
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
async fn rpc_cannot_substitute_another_transaction() {
    use alloy::network::TransactionResponse;
    use helios_common::execution_provider::TransactionProvider;
    use helios_core::execution::providers::rpc::RpcExecutionProvider;
    use jsonrpsee::{server::ServerBuilder, types::ErrorObjectOwned, RpcModule};
    let block = full_block();
    let tx = block.transactions.as_transactions().unwrap()[0].clone();
    let hash = tx.tx_hash();
    let server = ServerBuilder::default().build("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", server.local_addr().unwrap());
    let mut methods = RpcModule::new(tx);
    methods
        .register_method("eth_getTransactionByHash", |_, tx| {
            Ok::<_, ErrorObjectOwned>(tx.clone())
        })
        .unwrap();
    let handle = server.start(methods);
    let cache = BlockCache::<Ethereum>::new();
    cache
        .push_block(block.clone(), alloy::eips::BlockId::latest())
        .await;
    let provider = RpcExecutionProvider::<Ethereum, _, ()>::new(url.parse().unwrap(), cache);
    assert!(provider.get_transaction(hash).await.unwrap().is_some());
    assert!(provider
        .get_transaction(B256::ZERO)
        .await
        .unwrap()
        .is_none());
    assert!(provider
        .get_transaction_by_location(block.header.hash.into(), 1u64 << 32)
        .await
        .unwrap()
        .is_none());
    handle.stop().unwrap();
}
