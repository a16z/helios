use std::str::FromStr;

use alloy::primitives::B256;
use helios_consensus_core::{consensus_spec::MainnetConsensusSpec, types::BeaconBlock};
use reqwest::Client;
use serde_json::Value;
use tree_hash::TreeHash;

#[tokio::test]
async fn test_slot_12557247_consensus_rpc() {
    // Alternative test using consensus layer RPC
    let expected_hash =
        B256::from_str("0x9b9141d3c23f02ceb3fd5fac5ac4a299c7fd2ce2f3619270d973e5ce53ed18c9")
            .expect("Valid hash");

    // Try using a consensus RPC endpoint
    let consensus_rpc = "http://unstable.mainnet.beacon-api.nimbus.team";
    let endpoint = format!("{}/eth/v2/beacon/blocks/12557247", consensus_rpc);

    let client = Client::new();
    let response = client
        .get(&endpoint)
        .header("accept", "application/json")
        .send()
        .await
        .expect("rpc fetch failed");

     let json: Value = response.json().await.expect("Failed to parse JSON");

     // The block is under data.message
     let block_data = &json["data"]["message"];

     // Deserialize the block
     let beacon_block: BeaconBlock<MainnetConsensusSpec> =
         serde_json::from_value(block_data.clone())
             .expect("Failed to deserialize beacon block");

     // Calculate hash
     let calculated_hash = beacon_block.tree_hash_root();

     println!("Consensus RPC test:");
     println!("Slot: {}", beacon_block.slot);
     println!("Expected hash:   {}", expected_hash);
     println!("Calculated hash: {}", calculated_hash);

     assert_eq!(
         calculated_hash, expected_hash,
         "Tree hash root mismatch for slot 12557247"
     );

     println!("✓ Consensus RPC block hash verification successful!");
}
