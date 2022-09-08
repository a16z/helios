use std::str::FromStr;

use ethers::types::{Address, U256};
use ssz_rs::Vector;

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
    let address = Address::from_str("000095E79eAC4d76aab57cB2c1f091d553b36ca0").unwrap();

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
        U256::from_str_radix("145a440c047407cf0a", 16).unwrap()
    );
}

#[tokio::test]
async fn test_get_account_bad_proof() {
    let execution = get_client();
    let address = Address::from_str("000095E79eAC4d76aab57cB2c1f091d553b36ca0").unwrap();

    let mut payload = ExecutionPayload::default();
    payload.state_root = Vector::default();

    let account_res = execution.get_account(&address, None, &payload).await;

    assert!(account_res.is_err());
}
