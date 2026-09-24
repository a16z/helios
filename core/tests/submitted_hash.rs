use alloy::primitives::B256;
use helios_common::network_spec::NetworkSpec;
use helios_ethereum::spec::Ethereum;
use helios_test_utils::rpc_tx;
#[test]
fn rejects_wrong_submitted_transaction_hash() {
    use alloy::network::TransactionResponse;
    use helios_core::execution::providers::utils::verify_submitted_transaction_hash;
    let tx = rpc_tx();
    let encoded = Ethereum::encode_transaction(&tx);
    verify_submitted_transaction_hash::<Ethereum>(&encoded, tx.tx_hash()).unwrap();
    assert!(verify_submitted_transaction_hash::<Ethereum>(&encoded, B256::ZERO).is_err());
}
