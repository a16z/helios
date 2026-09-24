use alloy::{
    consensus::{
        proofs::calculate_transaction_root, proofs::calculate_withdrawals_root, TxReceipt,
    },
    rpc::types::{Block, BlockTransactions, Transaction},
};
use helios_common::{execution_provider::BlockProvider, network_spec::NetworkSpec};
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
#[tokio::test]
async fn rpc_cannot_omit_matching_logs() {
    use helios_common::execution_provider::LogProvider;
    use helios_core::execution::providers::rpc::RpcExecutionProvider;
    use jsonrpsee::{server::ServerBuilder, types::ErrorObjectOwned, RpcModule};

    let (block, receipt) = block_with_receipt();
    let tx = block.transactions.as_transactions().unwrap()[0].clone();
    let server = ServerBuilder::default().build("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", server.local_addr().unwrap());
    let mut methods = RpcModule::new((receipt.clone(), tx));
    methods
        .register_method("eth_getLogs", |_, _| {
            Ok::<_, ErrorObjectOwned>(Vec::<alloy::rpc::types::Log>::new())
        })
        .unwrap();
    methods
        .register_method("eth_getBlockReceipts", |_, ctx| {
            Ok::<_, ErrorObjectOwned>(vec![ctx.0.clone()])
        })
        .unwrap();
    methods
        .register_method("eth_getTransactionByHash", |_, ctx| {
            Ok::<_, ErrorObjectOwned>(ctx.1.clone())
        })
        .unwrap();
    let handle = server.start(methods);
    let cache = BlockCache::<Ethereum>::new();
    cache
        .push_block(block.clone(), alloy::eips::BlockId::latest())
        .await;
    let provider = RpcExecutionProvider::<Ethereum, _, ()>::new(url.parse().unwrap(), cache);
    let filter = alloy::rpc::types::Filter::new().at_block_hash(block.header.hash);
    let logs = provider.get_logs(&filter).await.unwrap();
    assert_eq!(logs, Ethereum::receipt_logs(&receipt));
    handle.stop().unwrap();
}
