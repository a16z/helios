use std::collections::BTreeMap;
use std::str::FromStr;

use consensus::types::primitives::{ByteList, ByteVector, U64};
use ethers::types::{Address, Filter, H256, U256};

use common::utils::hex_str_to_bytes;
use consensus::types::{ExecutionPayload, ExecutionPayloadBellatrix};
use execution::rpc::mock_rpc::MockRpc;
use execution::ExecutionClient;

fn get_client() -> ExecutionClient<MockRpc> {
    ExecutionClient::new("testdata/").unwrap()
}

#[tokio::test]
async fn test_get_account() {
    let execution = get_client();
    let address = Address::from_str("14f9D4aF749609c1438528C0Cce1cC3f6D411c47").unwrap();

    let payload = ExecutionPayload::Bellatrix(ExecutionPayloadBellatrix {
        state_root: ByteVector::try_from(
            hex_str_to_bytes("0xaa02f5db2ee75e3da400d10f3c30e894b6016ce8a2501680380a907b6674ce0d")
                .unwrap(),
        )
        .unwrap(),
        ..ExecutionPayloadBellatrix::default()
    });

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
async fn test_get_tx() {
    let execution = get_client();
    let tx_hash =
        H256::from_str("2dac1b27ab58b493f902dda8b63979a112398d747f1761c0891777c0983e591f").unwrap();

    let mut payload = ExecutionPayload::default();
    payload.transactions_mut().push(ByteList::try_from(hex_str_to_bytes("0x02f8b20583623355849502f900849502f91082ea6094326c977e6efc84e512bb9c30f76e30c160ed06fb80b844a9059cbb0000000000000000000000007daccf9b3c1ae2fa5c55f1c978aeef700bc83be0000000000000000000000000000000000000000000000001158e460913d00000c080a0e1445466b058b6f883c0222f1b1f3e2ad9bee7b5f688813d86e3fa8f93aa868ca0786d6e7f3aefa8fe73857c65c32e4884d8ba38d0ecfb947fbffb82e8ee80c167").unwrap()).unwrap());

    let mut payloads = BTreeMap::new();
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

    let mut payloads = BTreeMap::new();
    payloads.insert(7530933, payload);

    let tx_res = execution.get_transaction(&tx_hash, &payloads).await;

    assert!(tx_res.is_err());
}

#[tokio::test]
async fn test_get_tx_not_included() {
    let execution = get_client();
    let tx_hash =
        H256::from_str("2dac1b27ab58b493f902dda8b63979a112398d747f1761c0891777c0983e591f").unwrap();

    let payloads = BTreeMap::new();

    let tx_opt = execution
        .get_transaction(&tx_hash, &payloads)
        .await
        .unwrap();

    assert!(tx_opt.is_none());
}

#[tokio::test]
async fn test_get_logs() {
    let execution = get_client();
    let mut payload = ExecutionPayload::Bellatrix(ExecutionPayloadBellatrix {
        receipts_root: ByteVector::try_from(
            hex_str_to_bytes("dd82a78eccb333854f0c99e5632906e092d8a49c27a21c25cae12b82ec2a113f")
                .unwrap(),
        )
        .unwrap(),
        ..ExecutionPayloadBellatrix::default()
    });

    payload.transactions_mut().push(ByteList::try_from(hex_str_to_bytes("0x02f8b20583623355849502f900849502f91082ea6094326c977e6efc84e512bb9c30f76e30c160ed06fb80b844a9059cbb0000000000000000000000007daccf9b3c1ae2fa5c55f1c978aeef700bc83be0000000000000000000000000000000000000000000000001158e460913d00000c080a0e1445466b058b6f883c0222f1b1f3e2ad9bee7b5f688813d86e3fa8f93aa868ca0786d6e7f3aefa8fe73857c65c32e4884d8ba38d0ecfb947fbffb82e8ee80c167").unwrap()).unwrap());

    let mut payloads = BTreeMap::new();
    payloads.insert(7530933, payload);

    let filter = Filter::new();
    let logs = execution.get_logs(&filter, &payloads).await.unwrap();

    let tx_hash =
        H256::from_str("2dac1b27ab58b493f902dda8b63979a112398d747f1761c0891777c0983e591f").unwrap();

    assert!(!logs.is_empty());
    assert!(logs[0].transaction_hash.is_some());
    assert!(logs[0].transaction_hash.unwrap() == tx_hash);
}

#[tokio::test]
async fn test_get_receipt() {
    let execution = get_client();
    let tx_hash =
        H256::from_str("2dac1b27ab58b493f902dda8b63979a112398d747f1761c0891777c0983e591f").unwrap();

    let mut payload = ExecutionPayload::Bellatrix(ExecutionPayloadBellatrix {
        receipts_root: ByteVector::try_from(
            hex_str_to_bytes("dd82a78eccb333854f0c99e5632906e092d8a49c27a21c25cae12b82ec2a113f")
                .unwrap(),
        )
        .unwrap(),
        ..ExecutionPayloadBellatrix::default()
    });

    payload.transactions_mut().push(ByteList::try_from(hex_str_to_bytes("0x02f8b20583623355849502f900849502f91082ea6094326c977e6efc84e512bb9c30f76e30c160ed06fb80b844a9059cbb0000000000000000000000007daccf9b3c1ae2fa5c55f1c978aeef700bc83be0000000000000000000000000000000000000000000000001158e460913d00000c080a0e1445466b058b6f883c0222f1b1f3e2ad9bee7b5f688813d86e3fa8f93aa868ca0786d6e7f3aefa8fe73857c65c32e4884d8ba38d0ecfb947fbffb82e8ee80c167").unwrap()).unwrap());

    let mut payloads = BTreeMap::new();
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
    payload.transactions_mut().push(ByteList::try_from(hex_str_to_bytes("0x02f8b20583623355849502f900849502f91082ea6094326c977e6efc84e512bb9c30f76e30c160ed06fb80b844a9059cbb0000000000000000000000007daccf9b3c1ae2fa5c55f1c978aeef700bc83be0000000000000000000000000000000000000000000000001158e460913d00000c080a0e1445466b058b6f883c0222f1b1f3e2ad9bee7b5f688813d86e3fa8f93aa868ca0786d6e7f3aefa8fe73857c65c32e4884d8ba38d0ecfb947fbffb82e8ee80c167").unwrap()).unwrap());

    let mut payloads = BTreeMap::new();
    payloads.insert(7530933, payload);

    let receipt_res = execution.get_transaction_receipt(&tx_hash, &payloads).await;

    assert!(receipt_res.is_err());
}

#[tokio::test]
async fn test_get_receipt_not_included() {
    let execution = get_client();
    let tx_hash =
        H256::from_str("2dac1b27ab58b493f902dda8b63979a112398d747f1761c0891777c0983e591f").unwrap();

    let payloads = BTreeMap::new();
    let receipt_opt = execution
        .get_transaction_receipt(&tx_hash, &payloads)
        .await
        .unwrap();

    assert!(receipt_opt.is_none());
}

#[tokio::test]
async fn test_get_block() {
    let execution = get_client();
    let payload = ExecutionPayload::Bellatrix(ExecutionPayloadBellatrix {
        block_number: U64::from(12345),
        ..ExecutionPayloadBellatrix::default()
    });

    let block = execution.get_block(&payload, false).await.unwrap();

    assert_eq!(block.number, 12345);
}

#[tokio::test]
async fn test_get_tx_by_block_hash_and_index() {
    let execution = get_client();
    let tx_hash =
        H256::from_str("2dac1b27ab58b493f902dda8b63979a112398d747f1761c0891777c0983e591f").unwrap();

    let mut payload = ExecutionPayload::Bellatrix(ExecutionPayloadBellatrix {
        block_number: U64::from(7530933),
        ..ExecutionPayloadBellatrix::default()
    });
    payload.transactions_mut().push(ByteList::try_from(hex_str_to_bytes("0x02f8b20583623355849502f900849502f91082ea6094326c977e6efc84e512bb9c30f76e30c160ed06fb80b844a9059cbb0000000000000000000000007daccf9b3c1ae2fa5c55f1c978aeef700bc83be0000000000000000000000000000000000000000000000001158e460913d00000c080a0e1445466b058b6f883c0222f1b1f3e2ad9bee7b5f688813d86e3fa8f93aa868ca0786d6e7f3aefa8fe73857c65c32e4884d8ba38d0ecfb947fbffb82e8ee80c167").unwrap()).unwrap());

    let tx = execution
        .get_transaction_by_block_hash_and_index(&payload, 0)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(tx.hash(), tx_hash);
}
