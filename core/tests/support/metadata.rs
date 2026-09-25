use alloy::{
    consensus::{
        proofs::{calculate_transaction_root, calculate_withdrawals_root},
        transaction::SignerRecoverable,
        Receipt, ReceiptEnvelope, ReceiptWithBloom, SignableTransaction, Transaction as _,
        TxEip1559, TxEip2930, TxEip4844, TxEip7702, TxEnvelope, TxLegacy,
    },
    eips::eip4844::fake_exponential,
    primitives::{Address, TxKind, B256},
    rpc::types::{Block, BlockTransactions, Transaction, TransactionReceipt},
    signers::SignerSync,
};
use helios_common::{fork_schedule::ForkSchedule, network_spec::NetworkSpec};
use helios_core::execution::proof::{ordered_trie_root_noop_encoder, verify_block_receipts};
use helios_ethereum::spec::Ethereum;

pub(crate) fn forks() -> ForkSchedule {
    helios_ethereum::config::networks::Network::Mainnet
        .to_base_config()
        .execution_forks
}

pub(crate) fn envelopes(create: bool) -> Vec<TxEnvelope> {
    let signer: alloy::signers::local::PrivateKeySigner =
        "0000000000000000000000000000000000000000000000000000000000000001"
            .parse()
            .unwrap();
    let to = if create {
        TxKind::Create
    } else {
        TxKind::Call(Address::repeat_byte(2))
    };
    macro_rules! sign {
        ($tx:expr) => {{
            let tx = $tx;
            let sig = signer.sign_hash_sync(&tx.signature_hash()).unwrap();
            TxEnvelope::from(tx.into_signed(sig))
        }};
    }
    vec![
        sign!(TxLegacy {
            nonce: 7,
            to,
            gas_limit: 200000,
            gas_price: 1000,
            ..Default::default()
        }),
        sign!(TxEip2930 {
            nonce: 8,
            to,
            gas_limit: 200000,
            gas_price: 1000,
            ..Default::default()
        }),
        sign!(TxEip1559 {
            nonce: 9,
            to,
            gas_limit: 200000,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 10,
            ..Default::default()
        }),
        sign!(TxEip4844 {
            nonce: 10,
            to: Address::repeat_byte(2),
            gas_limit: 200000,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 10,
            max_fee_per_blob_gas: 10000,
            blob_versioned_hashes: vec![B256::repeat_byte(1)],
            ..Default::default()
        }),
        sign!(TxEip7702 {
            nonce: 11,
            to: Address::repeat_byte(2),
            gas_limit: 200000,
            max_fee_per_gas: 1000,
            max_priority_fee_per_gas: 10,
            ..Default::default()
        }),
    ]
}

pub(crate) fn fixture(
    txs: Vec<TxEnvelope>,
    timestamp: u64,
    denominator: u64,
    logs_per_tx: usize,
    success: bool,
) -> (Block<Transaction>, Vec<TransactionReceipt>) {
    let mut block = helios_test_utils::rpc_block();
    block.header.base_fee_per_gas = Some(100);
    block.header.timestamp = timestamp;
    block.header.excess_blob_gas = Some(20_000_000);
    block.header.transactions_root = calculate_transaction_root(&txs);
    block.header.withdrawals_root = Some(calculate_withdrawals_root(&[]));
    block.withdrawals = Some(Default::default());
    block.header.size = None;
    block.header.total_difficulty = None;
    block.header.blob_gas_used = Some(txs.iter().map(|t| t.blob_gas_used().unwrap_or(0)).sum());
    let mut cumulative = 0;
    let mut receipts = Vec::new();
    let template_log = helios_test_utils::rpc_tx_receipt().inner.logs()[0].clone();
    for (i, envelope) in txs.iter().enumerate() {
        let gas_used = 21000 + i as u64;
        cumulative += gas_used;
        let r = ReceiptWithBloom::from(Receipt {
            status: success.into(),
            cumulative_gas_used: cumulative,
            logs: vec![template_log.clone(); logs_per_tx],
        });
        let inner = match envelope {
            TxEnvelope::Legacy(_) => ReceiptEnvelope::Legacy(r),
            TxEnvelope::Eip2930(_) => ReceiptEnvelope::Eip2930(r),
            TxEnvelope::Eip1559(_) => ReceiptEnvelope::Eip1559(r),
            TxEnvelope::Eip4844(_) => ReceiptEnvelope::Eip4844(r),
            TxEnvelope::Eip7702(_) => ReceiptEnvelope::Eip7702(r),
        };
        let sender = envelope.recover_signer().unwrap();
        receipts.push(TransactionReceipt {
            inner,
            transaction_hash: *envelope.tx_hash(),
            transaction_index: Some(i as u64),
            block_hash: None,
            block_number: Some(block.header.number),
            gas_used,
            effective_gas_price: envelope.effective_gas_price(block.header.base_fee_per_gas),
            blob_gas_used: envelope.blob_gas_used(),
            blob_gas_price: envelope
                .blob_gas_used()
                .map(|_| fake_exponential(1, 20_000_000, denominator as u128)),
            from: sender,
            to: envelope.to(),
            contract_address: envelope
                .is_create()
                .then(|| sender.create(envelope.nonce())),
        });
    }
    block.header.gas_used = cumulative;
    block.header.receipts_root = ordered_trie_root_noop_encoder(
        &receipts
            .iter()
            .map(Ethereum::encode_receipt)
            .collect::<Vec<_>>(),
    );
    block.header.hash = block.header.hash_slow();
    let mut log_index = 0;
    for r in &mut receipts {
        r.block_hash = Some(block.header.hash);
        for log in &mut r.inner.as_receipt_with_bloom_mut().unwrap().receipt.logs {
            log.block_hash = r.block_hash;
            log.block_number = r.block_number;
            log.transaction_hash = Some(r.transaction_hash);
            log.transaction_index = r.transaction_index;
            log.log_index = Some(log_index);
            log.block_timestamp = Some(timestamp);
            log.removed = false;
            log_index += 1;
        }
    }
    block.transactions = BlockTransactions::Full(
        txs.into_iter()
            .enumerate()
            .map(|(i, t)| Transaction {
                inner: t.try_into_recovered().unwrap(),
                block_hash: Some(block.header.hash),
                block_number: Some(block.header.number),
                transaction_index: Some(i as u64),
                effective_gas_price: Some(receipts[i].effective_gas_price),
            })
            .collect(),
    );
    assert!(Ethereum::validate_block(&mut block, true));
    verify_block_receipts::<Ethereum>(&receipts, &block).unwrap();
    (block, receipts)
}
