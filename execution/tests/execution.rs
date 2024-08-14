use alloy::primitives::{U256, U64, b256, address};
use common::types::{Block, BlockTag};

use execution::rpc::mock_rpc::MockRpc;
use execution::state::State;
use execution::ExecutionClient;
use tokio::sync::mpsc::channel;
use tokio::sync::watch;

fn create_state() -> State {
    let (_, block_recv) = channel(256);
    let (_, finalized_recv) = watch::channel(None);
    State::new(block_recv, finalized_recv, 64)
}

fn create_client(state: State) -> ExecutionClient<MockRpc> {
    ExecutionClient::new("testdata/", state).unwrap()
}

#[tokio::test]
async fn test_get_account() {
    let state = create_state();

    let address = address!("14f9D4aF749609c1438528C0Cce1cC3f6D411c47");
    let block = Block {
        state_root: b256!("aa02f5db2ee75e3da400d10f3c30e894b6016ce8a2501680380a907b6674ce0d"),
        ..Default::default()
    };

    state.push_block(block).await;
    let execution = create_client(state);

    let account = execution
        .get_account(&address, None, BlockTag::Latest)
        .await
        .unwrap();

    assert_eq!(
        account.balance,
        U256::from_str_radix("48c27395000", 16).unwrap()
    );
}

#[tokio::test]
async fn test_get_account_bad_proof() {
    let state = create_state();

    let address = address!("14f9D4aF749609c1438528C0Cce1cC3f6D411c47");
    let block = Block::default();
    state.push_block(block).await;

    let execution = create_client(state);
    let account_res = execution
        .get_account(&address, None, BlockTag::Latest)
        .await;

    assert!(account_res.is_err());
}

// #[tokio::test]
// async fn test_get_tx() {
//     let state = create_state();
//     let tx_hash = b256!("2dac1b27ab58b493f902dda8b63979a112398d747f1761c0891777c0983e591f");
//
//     let mut block = Block::default();
//
//     let tx = Transaction::decode(&Rlp::new(&hex::decode("02f8b20583623355849502f900849502f91082ea6094326c977e6efc84e512bb9c30f76e30c160ed06fb80b844a9059cbb0000000000000000000000007daccf9b3c1ae2fa5c55f1c978aeef700bc83be0000000000000000000000000000000000000000000000001158e460913d00000c080a0e1445466b058b6f883c0222f1b1f3e2ad9bee7b5f688813d86e3fa8f93aa868ca0786d6e7f3aefa8fe73857c65c32e4884d8ba38d0ecfb947fbffb82e8ee80c167").unwrap())).unwrap();
//     let hash = tx.hash();
//     block.transactions = Transactions::Full(vec![tx]);
//
//     state.push_block(block).await;
//
//     let execution = create_client(state);
//     let tx = execution.get_transaction(hash).await.unwrap();
//
//     assert_eq!(tx.hash(), tx_hash);
// }

#[tokio::test]
async fn test_get_tx_not_included() {
    let state = create_state();
    let tx_hash = b256!("2dac1b27ab58b493f902dda8b63979a112398d747f1761c0891777c0983e591f");

    let block = Block::default();
    state.push_block(block).await;

    let execution = create_client(state);
    let tx_res = execution.get_transaction(tx_hash).await;

    assert!(tx_res.is_none());
}

// #[tokio::test]
// async fn test_get_logs() {
//     let tx = Transaction::decode(&Rlp::new(&hex::decode("02f8b20583623355849502f900849502f91082ea6094326c977e6efc84e512bb9c30f76e30c160ed06fb80b844a9059cbb0000000000000000000000007daccf9b3c1ae2fa5c55f1c978aeef700bc83be0000000000000000000000000000000000000000000000001158e460913d00000c080a0e1445466b058b6f883c0222f1b1f3e2ad9bee7b5f688813d86e3fa8f93aa868ca0786d6e7f3aefa8fe73857c65c32e4884d8ba38d0ecfb947fbffb82e8ee80c167").unwrap())).unwrap();
//
//     let block = Block {
//         number: 7530933.into(),
//         receipts_root: H256::from_str(
//             "dd82a78eccb333854f0c99e5632906e092d8a49c27a21c25cae12b82ec2a113f",
//         )
//         .unwrap(),
//         transactions: Transactions::Full(vec![tx]),
//         ..Default::default()
//     };
//
//     let state = create_state();
//     state.push_block(block).await;
//
//     let execution = create_client(state);
//     let filter = Filter::new();
//     let logs = execution.get_logs(&filter).await.unwrap();
//
//     let tx_hash =
//         H256::from_str("2dac1b27ab58b493f902dda8b63979a112398d747f1761c0891777c0983e591f").unwrap();
//
//     assert!(!logs.is_empty());
//     assert!(logs[0].transaction_hash.is_some());
//     assert!(logs[0].transaction_hash.unwrap() == tx_hash);
// }
//
// #[tokio::test]
// async fn test_get_receipt() {
//     let tx = Transaction::decode(&Rlp::new(&hex::decode("02f8b20583623355849502f900849502f91082ea6094326c977e6efc84e512bb9c30f76e30c160ed06fb80b844a9059cbb0000000000000000000000007daccf9b3c1ae2fa5c55f1c978aeef700bc83be0000000000000000000000000000000000000000000000001158e460913d00000c080a0e1445466b058b6f883c0222f1b1f3e2ad9bee7b5f688813d86e3fa8f93aa868ca0786d6e7f3aefa8fe73857c65c32e4884d8ba38d0ecfb947fbffb82e8ee80c167").unwrap())).unwrap();
//
//     let block = Block {
//         number: 7530933.into(),
//         receipts_root: H256::from_str(
//             "dd82a78eccb333854f0c99e5632906e092d8a49c27a21c25cae12b82ec2a113f",
//         )
//         .unwrap(),
//         transactions: Transactions::Full(vec![tx]),
//         ..Default::default()
//     };
//
//     let state = create_state();
//     state.push_block(block).await;
//     let execution = create_client(state);
//
//     let tx_hash =
//         H256::from_str("2dac1b27ab58b493f902dda8b63979a112398d747f1761c0891777c0983e591f").unwrap();
//
//     let receipt = execution
//         .get_transaction_receipt(&tx_hash)
//         .await
//         .unwrap()
//         .unwrap();
//
//     assert_eq!(receipt.transaction_hash, tx_hash);
// }
//
// #[tokio::test]
// async fn test_get_receipt_bad_proof() {
//     let tx = Transaction::decode(&Rlp::new(&hex::decode("02f8b20583623355849502f900849502f91082ea6094326c977e6efc84e512bb9c30f76e30c160ed06fb80b844a9059cbb0000000000000000000000007daccf9b3c1ae2fa5c55f1c978aeef700bc83be0000000000000000000000000000000000000000000000001158e460913d00000c080a0e1445466b058b6f883c0222f1b1f3e2ad9bee7b5f688813d86e3fa8f93aa868ca0786d6e7f3aefa8fe73857c65c32e4884d8ba38d0ecfb947fbffb82e8ee80c167").unwrap())).unwrap();
//
//     let block = Block {
//         number: 7530933.into(),
//         transactions: Transactions::Full(vec![tx]),
//         ..Default::default()
//     };
//
//     let state = create_state();
//     state.push_block(block).await;
//     let execution = create_client(state);
//
//     let tx_hash =
//         H256::from_str("2dac1b27ab58b493f902dda8b63979a112398d747f1761c0891777c0983e591f").unwrap();
//
//     let receipt_res = execution.get_transaction_receipt(&tx_hash).await;
//
//     assert!(receipt_res.is_err());
// }

#[tokio::test]
async fn test_get_receipt_not_included() {
    let state = create_state();
    let execution = create_client(state);
    let tx_hash = b256!("2dac1b27ab58b493f902dda8b63979a112398d747f1761c0891777c0983e591f");
    let receipt_opt = execution.get_transaction_receipt(&tx_hash).await.unwrap();

    assert!(receipt_opt.is_none());
}

#[tokio::test]
async fn test_get_block() {
    let block = Block {
        number: U64::from(12345),
        ..Default::default()
    };

    let state = create_state();
    state.push_block(block).await;
    let execution = create_client(state);

    let block = execution.get_block(BlockTag::Latest, false).await.unwrap();

    assert_eq!(block.number.to::<u64>(), 12345);
}

// #[tokio::test]
// async fn test_get_tx_by_block_hash_and_index() {
//     let tx = Transaction::decode(&Rlp::new(&hex::decode("02f8b20583623355849502f900849502f91082ea6094326c977e6efc84e512bb9c30f76e30c160ed06fb80b844a9059cbb0000000000000000000000007daccf9b3c1ae2fa5c55f1c978aeef700bc83be0000000000000000000000000000000000000000000000001158e460913d00000c080a0e1445466b058b6f883c0222f1b1f3e2ad9bee7b5f688813d86e3fa8f93aa868ca0786d6e7f3aefa8fe73857c65c32e4884d8ba38d0ecfb947fbffb82e8ee80c167").unwrap())).unwrap();
//
//     let block_hash =
//         H256::from_str("6663f197e991f5a0bb235f33ec554b9bd48c37b4f5002d7ac2abdfa99f86ac14").unwrap();
//
//     let block = Block {
//         number: 7530933.into(),
//         hash: block_hash,
//         receipts_root: H256::from_str(
//             "dd82a78eccb333854f0c99e5632906e092d8a49c27a21c25cae12b82ec2a113f",
//         )
//         .unwrap(),
//         transactions: Transactions::Full(vec![tx]),
//         ..Default::default()
//     };
//
//     let state = create_state();
//     state.push_block(block).await;
//     let execution = create_client(state);
//
//     let tx_hash =
//         H256::from_str("2dac1b27ab58b493f902dda8b63979a112398d747f1761c0891777c0983e591f").unwrap();
//
//     let tx = execution
//         .get_transaction_by_block_hash_and_index(block_hash, 0)
//         .await
//         .unwrap();
//
//     assert_eq!(tx.hash(), tx_hash);
// }
