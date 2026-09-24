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
async fn historical_blocks_must_match_the_requested_hash_or_height() {
    use helios_core::execution::providers::{
        historical::eip2935::Eip2935Provider, rpc::RpcExecutionProvider,
    };
    use jsonrpsee::{server::ServerBuilder, types::ErrorObjectOwned, RpcModule};
    let block = full_block();
    let server = ServerBuilder::default().build("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", server.local_addr().unwrap());
    let mut methods = RpcModule::new(block.clone());
    for name in ["eth_getBlockByHash", "eth_getBlockByNumber"] {
        methods
            .register_method(name, |_, block| Ok::<_, ErrorObjectOwned>(block.clone()))
            .unwrap();
    }
    let handle = server.start(methods);
    let provider = RpcExecutionProvider::<Ethereum, _, _>::with_historical_provider(
        url.parse().unwrap(),
        BlockCache::new(),
        Eip2935Provider::new(),
    );
    for id in [
        B256::ZERO.into(),
        alloy::eips::BlockId::number(block.header.number - 1),
    ] {
        let err = provider.get_block(id, false).await.unwrap_err();
        assert!(
            err.to_string().contains("does not match requested block"),
            "{err}"
        );
    }
    handle.stop().unwrap();
}
