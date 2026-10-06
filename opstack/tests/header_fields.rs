use alloy::{
    consensus::proofs::calculate_withdrawals_root,
    primitives::{b256, B256},
    rpc::types::{Block, BlockTransactions},
};
use helios_common::network_spec::NetworkSpec;
use helios_ethereum::spec::Ethereum;
use helios_opstack::spec::OpStack;
use serde::{de::DeserializeOwned, Serialize};

fn check_header_fields<N, T>(withdrawals_root: B256)
where
    N: NetworkSpec<BlockResponse = Block<T>>,
    T: Clone + Serialize + DeserializeOwned,
{
    let empty_requests = b256!("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    for requests_hash in [None, Some(empty_requests)] {
        let mut original = Block::<T>::new(Default::default(), BlockTransactions::Full(vec![]))
            .with_withdrawals(Some(Default::default()));
        original.header.base_fee_per_gas = Some(1);
        original.header.withdrawals_root = Some(if requests_hash.is_some() {
            withdrawals_root
        } else {
            calculate_withdrawals_root(&[])
        });
        original.header.blob_gas_used = Some(0);
        original.header.excess_blob_gas = Some(0);
        original.header.parent_beacon_block_root = Some(B256::repeat_byte(2));
        original.header.requests_hash = requests_hash;
        original.header.hash = original.header.hash_slow();
        assert!(N::validate_block(&mut original.clone(), true));

        let mut malformed = original.clone();
        if requests_hash.is_some() {
            malformed.header.block_access_list_hash = malformed.header.requests_hash.take();
        } else {
            malformed.header.requests_hash = malformed.header.parent_beacon_block_root.take();
        }
        // RPC decoding accepts the gap, while RLP encoding skips the absent field.
        let json = serde_json::to_string(&malformed).unwrap();
        let malformed: Block<T> = serde_json::from_str(&json).unwrap();
        assert_eq!(
            alloy::rlp::encode(&malformed.header.inner),
            alloy::rlp::encode(&original.header.inner)
        );
        assert_eq!(malformed.header.hash_slow(), original.header.hash);
        for full_tx in [false, true] {
            assert!(
                !N::validate_block(&mut malformed.clone(), full_tx),
                "{} accepted relocated field (requests_hash: {requests_hash:?}, full_tx: {full_tx})",
                std::any::type_name::<N>()
            );
        }
    }
}

#[test]
fn rpc_blocks_reject_relocated_optional_header_fields() {
    // Isthmus uses the message passer storage root, not the withdrawals trie root.
    check_header_fields::<OpStack, op_alloy_rpc_types::Transaction>(B256::repeat_byte(6));
    check_header_fields::<Ethereum, alloy::rpc::types::Transaction>(
        calculate_withdrawals_root(&[]),
    );
}
