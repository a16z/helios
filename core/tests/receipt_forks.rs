#[path = "support/metadata.rs"]
mod metadata;
use alloy::{
    eips::eip4844::fake_exponential,
    rpc::types::{Block, Transaction, TransactionReceipt},
};
use helios_common::network_spec::NetworkSpec;
use helios_core::execution::proof::{verify_authenticated_block_receipts, verify_block_receipts};
use helios_ethereum::spec::Ethereum;
use metadata::{envelopes, fixture, forks};

#[test]
fn active_ethereum_networks_use_their_own_bpo_activation_times() {
    use helios_ethereum::config::networks::Network;
    // Mainnet EIP-8134/8135; testnet times from go-ethereum's network configs.
    for (network, bpo1, bpo2) in [
        (Network::Mainnet, 1765290071, 1767747671),
        (Network::Sepolia, 1761017184, 1761607008),
        (Network::Hoodi, 1762365720, 1762955544),
    ] {
        let schedule = network.to_base_config().execution_forks;
        for (time, expected) in [
            (bpo1 - 1, 5007716),
            (bpo1, 8346193),
            (bpo2 - 1, 8346193),
            (bpo2, 11684671),
        ] {
            assert_eq!(
                schedule.get_blob_base_fee_update_fraction(time),
                expected,
                "{network} at {time}"
            );
        }
    }
}
#[test]
fn all_types_creation_failure_forks_and_global_log_indices() {
    for (timestamp, denominator) in [
        (1710338135, 3338477),
        (1746612311, 5007716),
        (1764798551, 5007716),
    ] {
        for create in [false, true] {
            for success in [false, true] {
                let (block, receipts) =
                    fixture(envelopes(create), timestamp, denominator, 3, success);
                verify_authenticated_block_receipts::<Ethereum>(&receipts, &block, &forks())
                    .unwrap();
                let mut forged = receipts.clone();
                forged[1].gas_used += 1;
                verify_block_receipts::<Ethereum>(&forged, &block).unwrap();
                assert!(
                    verify_authenticated_block_receipts::<Ethereum>(&forged, &block, &forks())
                        .is_err()
                );
                let mut forged = receipts.clone();
                forged[1]
                    .inner
                    .as_receipt_with_bloom_mut()
                    .unwrap()
                    .receipt
                    .logs[0]
                    .log_index = Some(0);
                verify_block_receipts::<Ethereum>(&forged, &block).unwrap();
                assert!(
                    verify_authenticated_block_receipts::<Ethereum>(&forged, &block, &forks())
                        .is_err()
                );
            }
        }
    }
    let (block, receipts) = fixture(vec![], 1746612311, 5007716, 0, true);
    verify_authenticated_block_receipts::<Ethereum>(&receipts, &block, &forks()).unwrap();
}

#[test]
fn bpo1_correct_blob_fee_must_be_accepted() {
    let (block, receipts) = fixture(envelopes(false), 1765290071, 8346193, 1, true);
    assert_eq!(receipts[3].blob_gas_price, Some(10));
    verify_authenticated_block_receipts::<Ethereum>(&receipts, &block, &forks()).unwrap();
}

#[test]
fn bpo2_correct_blob_fee_must_be_accepted() {
    let (block, receipts) = fixture(envelopes(false), 1767747671, 11684671, 1, true);
    assert_eq!(receipts[3].blob_gas_price, Some(5));
    verify_authenticated_block_receipts::<Ethereum>(&receipts, &block, &forks()).unwrap();
}

#[test]
fn bpo2_forged_prague_blob_fee_must_be_rejected() {
    let (block, mut receipts) = fixture(envelopes(false), 1767747671, 11684671, 1, true);
    let original = Ethereum::encode_receipt(&receipts[3]);
    receipts[3].blob_gas_price = Some(fake_exponential(1, 20_000_000, 5007716));
    assert_eq!(original, Ethereum::encode_receipt(&receipts[3]));
    assert!(
        verify_authenticated_block_receipts::<Ethereum>(&receipts, &block, &forks()).is_err(),
        "accepted forged blobGasPrice=54 when the correct BPO2 price is 5"
    );
}

#[test]
fn real_mainnet_bpo2_block_receipts_must_be_accepted() {
    let mut block: Block<Transaction> =
        serde_json::from_str(include_str!("../../tests/testdata/rpc/bpo2/block.json")).unwrap();
    let receipts: Vec<TransactionReceipt> =
        serde_json::from_str(include_str!("../../tests/testdata/rpc/bpo2/receipts.json")).unwrap();
    assert!(
        Ethereum::validate_block(&mut block, true),
        "real block body and tx metadata failed validation"
    );
    verify_block_receipts::<Ethereum>(&receipts, &block).unwrap();
    for receipt in &receipts {
        if let Some(price) = receipt.blob_gas_price {
            let excess = block.header.excess_blob_gas.unwrap() as u128;
            assert_eq!(price, fake_exponential(1, excess, 11684671));
            assert_ne!(price, fake_exponential(1, excess, 5007716));
        }
    }
    verify_authenticated_block_receipts::<Ethereum>(&receipts, &block, &forks()).unwrap();
}
