use alloy::{
    consensus::{
        proofs::calculate_transaction_root,
        transaction::{Recovered, SignerRecoverable},
        Sealed, SignableTransaction, Signed, Transaction as _, TxEip1559,
    },
    primitives::{Address, B256},
    rpc::types::{Block, BlockTransactions},
};
use helios_common::network_spec::NetworkSpec;
use helios_opstack::spec::OpStack;
use op_alloy_consensus::{OpTxEnvelope, TxDeposit};
use op_alloy_rpc_types::Transaction;

fn full_block(deposit: bool) -> Block<Transaction> {
    let envelope: OpTxEnvelope = if deposit {
        TxDeposit {
            from: Address::repeat_byte(1),
            ..Default::default()
        }
        .into()
    } else {
        use alloy::signers::SignerSync;
        let signer: alloy::signers::local::PrivateKeySigner =
            "0000000000000000000000000000000000000000000000000000000000000001"
                .parse()
                .unwrap();
        let tx = TxEip1559 {
            max_fee_per_gas: 100_000_000_000,
            max_priority_fee_per_gas: 1_000_000_000,
            ..Default::default()
        };
        let signature = signer.sign_hash_sync(&tx.signature_hash()).unwrap();
        tx.into_signed(signature).into()
    };
    let mut block: Block<Transaction> = Default::default();
    block.header.base_fee_per_gas = Some(100_000_000);
    block.header.transactions_root = calculate_transaction_root(std::slice::from_ref(&envelope));
    block.withdrawals = Some(Default::default());
    block.header.hash = block.header.hash_slow();
    block.transactions = BlockTransactions::Full(vec![Transaction {
        inner: alloy::rpc::types::Transaction {
            inner: envelope.try_into_recovered().unwrap(),
            block_hash: Some(block.header.hash),
            block_number: Some(0),
            transaction_index: Some(0),
            effective_gas_price: Some(u128::MAX),
        },
        deposit_nonce: Some(9999),
        deposit_receipt_version: Some(9999),
    }]);
    block
}

#[test]
fn accepts_deposits_and_signed_transactions_and_normalizes_unproven_fields() {
    for deposit in [false, true] {
        let mut block = full_block(deposit);
        assert!(OpStack::validate_block(&mut block, true));
        let tx = &block.transactions.as_transactions().unwrap()[0];
        assert_eq!(
            tx.inner.effective_gas_price,
            Some(tx.effective_gas_price(block.header.base_fee_per_gas))
        );
        if deposit {
            assert_eq!(tx.inner.effective_gas_price, Some(0));
        }
        assert!(tx.deposit_nonce.is_none());
        assert!(tx.deposit_receipt_version.is_none());
    }
}

#[test]
fn rejects_deposit_and_signed_transaction_forged_hash_sender_and_location() {
    for deposit in [false, true] {
        for field in [
            "sender",
            "hash",
            "blockHash",
            "blockNumber",
            "transactionIndex",
        ] {
            let mut block = full_block(deposit);
            let expected: Vec<_> = block.transactions.hashes().collect();
            let BlockTransactions::Full(txs) = &mut block.transactions else {
                panic!()
            };
            let tx = &mut txs[0];
            if field == "sender" {
                tx.inner.inner =
                    Recovered::new_unchecked(tx.inner.inner.clone().into_inner(), Address::ZERO);
            } else if field == "hash" {
                let signer = tx.inner.inner.signer();
                let envelope = match tx.inner.inner.clone().into_inner() {
                    OpTxEnvelope::Deposit(t) => {
                        OpTxEnvelope::Deposit(Sealed::new_unchecked(t.into_inner(), B256::ZERO))
                    }
                    OpTxEnvelope::Eip1559(t) => OpTxEnvelope::Eip1559(Signed::new_unchecked(
                        t.tx().clone(),
                        *t.signature(),
                        B256::ZERO,
                    )),
                    _ => unreachable!(),
                };
                tx.inner.inner = Recovered::new_unchecked(envelope, signer);
            } else {
                match field {
                    "blockHash" => tx.inner.block_hash = Some(B256::ZERO),
                    "blockNumber" => tx.inner.block_number = Some(99),
                    "transactionIndex" => tx.inner.transaction_index = Some(99),
                    _ => unreachable!(),
                }
            }
            assert!(OpStack::is_hash_valid(&block));
            assert!(
                !OpStack::validate_block(&mut block, true),
                "accepted {field}, deposit={deposit}"
            );
            assert!(OpStack::validate_block(&mut block, false));
            assert_eq!(block.transactions, BlockTransactions::Hashes(expected));
        }
    }
}
