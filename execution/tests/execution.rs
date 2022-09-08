use std::collections::HashMap;
use std::str::FromStr;

use ethers::types::{Address, H256, U256};
use ethers::utils::keccak256;
use ssz_rs::{List, Vector};

use common::utils::hex_str_to_bytes;
use consensus::types::ExecutionPayload;
use execution::rpc::mock_rpc::MockRpc;
use execution::ExecutionClient;

fn get_client() -> ExecutionClient<MockRpc> {
    ExecutionClient::new("testdata/").unwrap()
}

#[tokio::test]
async fn test_get_account() {
    let execution = get_client();
    let address = Address::from_str("14f9D4aF749609c1438528C0Cce1cC3f6D411c47").unwrap();

    let mut payload = ExecutionPayload::default();
    payload.state_root = Vector::from_iter(
        hex_str_to_bytes("0xaa02f5db2ee75e3da400d10f3c30e894b6016ce8a2501680380a907b6674ce0d")
            .unwrap(),
    );

    let account = execution
        .get_account(&address, None, &payload)
        .await
        .unwrap();

    assert_eq!(
        account.balance,
        U256::from_str_radix("48c27395000", 16).unwrap()
    );
}

#[tokio::test]
async fn test_get_account_bad_proof() {
    let execution = get_client();
    let address = Address::from_str("14f9D4aF749609c1438528C0Cce1cC3f6D411c47").unwrap();
    let payload = ExecutionPayload::default();

    let account_res = execution.get_account(&address, None, &payload).await;

    assert!(account_res.is_err());
}

#[tokio::test]
async fn test_get_code() {
    let execution = get_client();
    let address = Address::from_str("14f9D4aF749609c1438528C0Cce1cC3f6D411c47").unwrap();

    let mut payload = ExecutionPayload::default();
    payload.state_root = Vector::from_iter(
        hex_str_to_bytes("0xaa02f5db2ee75e3da400d10f3c30e894b6016ce8a2501680380a907b6674ce0d")
            .unwrap(),
    );

    let code = execution.get_code(&address, &payload).await.unwrap();
    let code_hash = keccak256(code);

    assert_eq!(
        code_hash.as_slice(),
        &hex_str_to_bytes("0xc6ca0679d7242fa080596f2fe2e6b172d9b927a6b52278343826e33745854327")
            .unwrap()
    );
}

#[tokio::test]
async fn test_get_code_bad_proof() {
    let execution = get_client();
    let address = Address::from_str("14f9D4aF749609c1438528C0Cce1cC3f6D411c47").unwrap();

    let payload = ExecutionPayload::default();

    let code_res = execution.get_code(&address, &payload).await;

    assert!(code_res.is_err());
}

#[tokio::test]
async fn test_get_tx() {
    let execution = get_client();
    let tx_hash =
        H256::from_str("2dac1b27ab58b493f902dda8b63979a112398d747f1761c0891777c0983e591f").unwrap();

    let mut payload = ExecutionPayload::default();
    payload.transactions.push(List::from_iter(hex_str_to_bytes("0x02f8b20583623355849502f900849502f91082ea6094326c977e6efc84e512bb9c30f76e30c160ed06fb80b844a9059cbb0000000000000000000000007daccf9b3c1ae2fa5c55f1c978aeef700bc83be0000000000000000000000000000000000000000000000001158e460913d00000c080a0e1445466b058b6f883c0222f1b1f3e2ad9bee7b5f688813d86e3fa8f93aa868ca0786d6e7f3aefa8fe73857c65c32e4884d8ba38d0ecfb947fbffb82e8ee80c167").unwrap()));

    let mut payloads = HashMap::new();
    payloads.insert(7530933, payload);

    let tx = execution
        .get_transaction(&tx_hash, &payloads)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(tx.hash(), tx_hash);
}

#[tokio::test]
async fn test_get_tx_bad_proof() {
    let execution = get_client();
    let tx_hash =
        H256::from_str("2dac1b27ab58b493f902dda8b63979a112398d747f1761c0891777c0983e591f").unwrap();

    let payload = ExecutionPayload::default();
    let mut payloads = HashMap::new();
    payloads.insert(7530933, payload);

    let tx_res = execution.get_transaction(&tx_hash, &payloads).await;

    assert!(tx_res.is_err());
}

#[tokio::test]
async fn test_get_tx_not_included() {
    let execution = get_client();
    let tx_hash =
        H256::from_str("2dac1b27ab58b493f902dda8b63979a112398d747f1761c0891777c0983e591f").unwrap();

    let payloads = HashMap::new();

    let tx_opt = execution
        .get_transaction(&tx_hash, &payloads)
        .await
        .unwrap();

    assert!(tx_opt.is_none());
}

#[tokio::test]
async fn test_get_receipt() {
    let execution = get_client();
    let tx_hash =
        H256::from_str("2dac1b27ab58b493f902dda8b63979a112398d747f1761c0891777c0983e591f").unwrap();

    let mut payload = ExecutionPayload::default();

    payload.receipts_root = Vector::from_iter(
        hex_str_to_bytes("dd82a78eccb333854f0c99e5632906e092d8a49c27a21c25cae12b82ec2a113f")
            .unwrap(),
    );

    payload.transactions.push(List::from_iter(hex_str_to_bytes("0x02f8b20583623355849502f900849502f91082ea6094326c977e6efc84e512bb9c30f76e30c160ed06fb80b844a9059cbb0000000000000000000000007daccf9b3c1ae2fa5c55f1c978aeef700bc83be0000000000000000000000000000000000000000000000001158e460913d00000c080a0e1445466b058b6f883c0222f1b1f3e2ad9bee7b5f688813d86e3fa8f93aa868ca0786d6e7f3aefa8fe73857c65c32e4884d8ba38d0ecfb947fbffb82e8ee80c167").unwrap()));

    let mut payloads = HashMap::new();
    payloads.insert(7530933, payload);

    let receipt = execution
        .get_transaction_receipt(&tx_hash, &payloads)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(receipt.transaction_hash, tx_hash);
}

#[tokio::test]
async fn test_get_receipt_bad_proof() {
    let execution = get_client();
    let tx_hash =
        H256::from_str("2dac1b27ab58b493f902dda8b63979a112398d747f1761c0891777c0983e591f").unwrap();

    let mut payload = ExecutionPayload::default();
    payload.transactions.push(List::from_iter(hex_str_to_bytes("0x02f8b20583623355849502f900849502f91082ea6094326c977e6efc84e512bb9c30f76e30c160ed06fb80b844a9059cbb0000000000000000000000007daccf9b3c1ae2fa5c55f1c978aeef700bc83be0000000000000000000000000000000000000000000000001158e460913d00000c080a0e1445466b058b6f883c0222f1b1f3e2ad9bee7b5f688813d86e3fa8f93aa868ca0786d6e7f3aefa8fe73857c65c32e4884d8ba38d0ecfb947fbffb82e8ee80c167").unwrap()));

    let mut payloads = HashMap::new();
    payloads.insert(7530933, payload);

    let receipt_res = execution.get_transaction_receipt(&tx_hash, &payloads).await;

    assert!(receipt_res.is_err());
}

#[tokio::test]
async fn test_get_receipt_not_included() {
    let execution = get_client();
    let tx_hash =
        H256::from_str("2dac1b27ab58b493f902dda8b63979a112398d747f1761c0891777c0983e591f").unwrap();

    let payloads = HashMap::new();
    let receipt_opt = execution
        .get_transaction_receipt(&tx_hash, &payloads)
        .await
        .unwrap();

    assert!(receipt_opt.is_none());
}
