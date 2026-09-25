use super::ExecutionRequests;
use crate::consensus_spec::MainnetConsensusSpec;
use alloy::primitives::b256;
use serde_json::json;

#[test]
fn empty_execution_requests_use_sha256_of_empty_bytes() {
    assert_eq!(
        ExecutionRequests::<MainnetConsensusSpec>::default().requests_hash(),
        b256!("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
    );
}

#[test]
fn execution_requests_hash_all_three_types_in_order() {
    let requests: ExecutionRequests<MainnetConsensusSpec> = serde_json::from_value(json!({
        "deposits": [{
            "pubkey": format!("0x{}", "01".repeat(48)),
            "withdrawal_credentials": format!("0x{}", "02".repeat(32)),
            "amount": "3",
            "signature": format!("0x{}", "04".repeat(96)),
            "index": "5"
        }],
        "withdrawals": [{
            "source_address": format!("0x{}", "06".repeat(20)),
            "validator_pubkey": format!("0x{}", "07".repeat(48)),
            "amount": "8"
        }],
        "consolidations": [{
            "source_address": format!("0x{}", "09".repeat(20)),
            "source_pubkey": format!("0x{}", "0a".repeat(48)),
            "target_pubkey": format!("0x{}", "0b".repeat(48))
        }]
    }))
    .unwrap();
    // Independently calculated from type-prefixed fixed-width records, with
    // little-endian u64 values, using the EIP-7685 SHA-256 commitment.
    assert_eq!(
        requests.requests_hash(),
        b256!("efb32b64dd105cd70368f709de6c55d7dc0411cb25b0214d9dfd68607fb9b4df")
    );
}
