use alloy::{
    consensus::{
        proofs::calculate_transaction_root, transaction::SignerRecoverable, Receipt,
        ReceiptWithBloom, SignableTransaction, Transaction as _, TxEip1559,
    },
    network::TransactionResponse,
    primitives::{Address, TxKind},
    rpc::types::{Block, BlockTransactions, TransactionReceipt},
    signers::SignerSync,
};
use helios_common::network_spec::NetworkSpec;
use helios_core::execution::proof::{
    ordered_trie_root_noop_encoder, verify_authenticated_block_receipts, verify_block_receipts,
};
use helios_opstack::spec::OpStack;
use op_alloy_consensus::{OpDepositReceipt, OpReceipt, OpTxEnvelope, TxDeposit};
use op_alloy_rpc_types::{OpTransactionReceipt, Transaction};

fn fixture(
    deposit: bool,
    nonce: Option<u64>,
    success: bool,
) -> (Block<Transaction>, OpTransactionReceipt) {
    let envelope: OpTxEnvelope = if deposit {
        TxDeposit {
            from: Address::repeat_byte(1),
            to: TxKind::Create,
            gas_limit: 100000,
            ..Default::default()
        }
        .into()
    } else {
        let signer: alloy::signers::local::PrivateKeySigner =
            "0000000000000000000000000000000000000000000000000000000000000001"
                .parse()
                .unwrap();
        let tx = TxEip1559 {
            nonce: 7,
            to: TxKind::Create,
            gas_limit: 100000,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 10,
            ..Default::default()
        };
        let sig = signer.sign_hash_sync(&tx.signature_hash()).unwrap();
        tx.into_signed(sig).into()
    };
    let mut block: Block<Transaction> = Default::default();
    block.header.base_fee_per_gas = Some(100);
    block.header.transactions_root = calculate_transaction_root(std::slice::from_ref(&envelope));
    block.withdrawals = Some(Default::default());
    let tx = Transaction {
        inner: alloy::rpc::types::Transaction {
            inner: envelope.try_into_recovered().unwrap(),
            block_hash: None,
            block_number: Some(0),
            transaction_index: Some(0),
            block_timestamp: None,
            effective_gas_price: None,
        },
        deposit_nonce: None,
        deposit_receipt_version: None,
    };
    let r = Receipt {
        status: success.into(),
        cumulative_gas_used: 53000,
        logs: vec![],
    };
    let inner = if deposit {
        OpReceipt::Deposit(OpDepositReceipt {
            inner: r,
            deposit_nonce: nonce,
            deposit_receipt_version: nonce.map(|_| 1),
        })
    } else {
        OpReceipt::Eip1559(r)
    };
    let mut receipt = OpTransactionReceipt {
        inner: TransactionReceipt {
            inner: ReceiptWithBloom::from(inner),
            transaction_hash: tx.tx_hash(),
            transaction_index: Some(0),
            block_hash: None,
            block_number: Some(0),
            gas_used: 53000,
            effective_gas_price: tx.effective_gas_price(block.header.base_fee_per_gas),
            blob_gas_used: None,
            blob_gas_price: None,
            from: tx.from(),
            to: None,
            contract_address: Some(tx.from().create(nonce.unwrap_or_else(|| tx.nonce()))),
        },
        l1_block_info: Default::default(),
        op_gas_refund: None,
    };
    block.header.receipts_root =
        ordered_trie_root_noop_encoder(&[OpStack::encode_receipt(&receipt)]);
    block.header.gas_used = receipt.inner.gas_used;
    block.header.hash = block.header.hash_slow();
    let mut tx = tx;
    tx.inner.block_hash = Some(block.header.hash);
    receipt.inner.block_hash = Some(block.header.hash);
    block.transactions = BlockTransactions::Full(vec![tx]);
    assert!(OpStack::validate_block(&mut block, true));
    verify_authenticated_block_receipts::<OpStack>(
        std::slice::from_ref(&receipt),
        &block,
        &Default::default(),
    )
    .unwrap();
    (block, receipt)
}

#[test]
fn op_signed_and_deposit_creation_uses_authenticated_nonce_including_failed_creation() {
    for (deposit, nonce) in [(false, None), (true, None), (true, Some(42))] {
        for success in [false, true] {
            let (block, receipt) = fixture(deposit, nonce, success);
            for field in ["contract", "gas", "price", "sender", "blob"] {
                let mut forged = receipt.clone();
                match field {
                    "contract" => forged.inner.contract_address = Some(Address::ZERO),
                    "gas" => forged.inner.gas_used += 1,
                    "price" => forged.inner.effective_gas_price += 1,
                    "sender" => forged.inner.from = Address::ZERO,
                    "blob" => forged.inner.blob_gas_price = Some(1),
                    _ => unreachable!(),
                }
                assert_eq!(
                    OpStack::encode_receipt(&receipt),
                    OpStack::encode_receipt(&forged)
                );
                verify_block_receipts::<OpStack>(std::slice::from_ref(&forged), &block).unwrap();
                assert!(
                    verify_authenticated_block_receipts::<OpStack>(
                        &[forged],
                        &block,
                        &Default::default()
                    )
                    .is_err(),
                    "accepted {field}"
                );
            }
        }
    }
}

#[tokio::test]
async fn both_receipt_apis_strip_all_unverified_fees_without_extra_rpc_calls() {
    use alloy::eips::BlockId;
    use helios_common::execution_provider::{BlockProvider, ReceiptProvider};
    use helios_core::execution::providers::{
        block::block_cache::BlockCache, rpc::RpcExecutionProvider,
    };
    use jsonrpsee::{server::ServerBuilder, types::ErrorObjectOwned, RpcModule};
    use std::sync::{Arc, Mutex};

    let (block, mut receipt) = fixture(false, None, true);
    receipt.op_gas_refund = Some(u64::MAX);
    receipt.l1_block_info = op_alloy_rpc_types::L1BlockInfo {
        l1_gas_price: Some(u128::MAX),
        l1_gas_used: Some(u128::MAX),
        l1_fee: Some(u128::MAX),
        l1_fee_scalar: Some(123456.0),
        l1_base_fee_scalar: Some(u128::MAX),
        l1_blob_base_fee: Some(u128::MAX),
        l1_blob_base_fee_scalar: Some(u128::MAX),
        operator_fee_scalar: Some(u128::MAX),
        operator_fee_constant: Some(u128::MAX),
        da_footprint_gas_scalar: Some(u16::MAX),
    };
    let original_encoding = OpStack::encode_receipt(&receipt);
    let hash = receipt.inner.transaction_hash;
    let block_hash = block.header.hash;
    let calls = Arc::new(Mutex::new(Vec::new()));
    let server = ServerBuilder::default().build("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", server.local_addr().unwrap());
    let mut rpc = RpcModule::new(calls.clone());
    let locator = receipt.clone();
    rpc.register_method("eth_getTransactionReceipt", move |_, calls| {
        calls.lock().unwrap().push("locator");
        Ok::<_, ErrorObjectOwned>(Some(locator.clone()))
    })
    .unwrap();
    rpc.register_method("eth_getBlockReceipts", move |params, calls| {
        assert_eq!(params.one::<BlockId>()?, BlockId::from(block_hash));
        calls.lock().unwrap().push("receipts");
        Ok::<_, ErrorObjectOwned>(Some(vec![receipt.clone()]))
    })
    .unwrap();
    let handle = server.start(rpc);
    let cache = BlockCache::new();
    cache.push_block(block, BlockId::latest()).await;
    let provider = RpcExecutionProvider::<OpStack, _, ()>::new(
        url.parse().unwrap(),
        cache,
        Default::default(),
    );
    let individual = provider.get_receipt(hash).await.unwrap().unwrap();
    assert_eq!(*calls.lock().unwrap(), vec!["locator", "receipts"]);
    calls.lock().unwrap().clear();
    let receipts = provider
        .get_block_receipts(block_hash.into())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(*calls.lock().unwrap(), vec!["receipts"]);
    for clean in std::iter::once(individual).chain(receipts) {
        assert_eq!(clean.l1_block_info, Default::default());
        assert!(clean.op_gas_refund.is_none());
        assert_eq!(OpStack::encode_receipt(&clean), original_encoding);
    }
    handle.stop().unwrap();
}

#[test]
fn receipt_trie_encoding_matches_eip2718_envelopes() {
    use alloy::eips::Encodable2718;
    for deposit in [false, true] {
        let (_, receipt) = fixture(deposit, deposit.then_some(42), true);
        let envelope: op_alloy_consensus::OpReceiptEnvelope = receipt
            .inner
            .inner
            .clone()
            .map_receipt(|receipt| receipt.map_logs(|log| log.inner))
            .into();
        assert_eq!(OpStack::encode_receipt(&receipt), envelope.encoded_2718());
    }
}
