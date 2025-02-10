use alloy::{
    network::ReceiptResponse,
    primitives::{Bytes, B256},
    rlp,
};
use alloy_trie::{
    proof::{verify_proof as mpt_verify_proof, ProofRetainer},
    root::adjust_index_for_rlp,
    HashBuilder, Nibbles,
};

use helios_core::network_spec::NetworkSpec;

/// Create a MPT proof for a given receipt in a list of receipts.
pub fn create_receipt_proof<N: NetworkSpec>(
    receipts: Vec<N::ReceiptResponse>,
    target_index: usize,
) -> Vec<Bytes> {
    // Initialise the trie builder with proof retainer for the target index
    let receipts_len = receipts.len();
    let target_index = adjust_index_for_rlp(target_index, receipts_len);
    let retainer = ProofRetainer::new(vec![Nibbles::unpack(rlp::encode_fixed_size(&target_index))]);
    let mut hb = HashBuilder::default().with_proof_retainer(retainer);

    // Iterate over each receipt, adding it to the trie
    for i in 0..receipts_len {
        let index = adjust_index_for_rlp(i, receipts_len);
        let index_buffer = rlp::encode_fixed_size(&index);
        hb.add_leaf(
            Nibbles::unpack(&index_buffer),
            N::encode_receipt(&receipts[index]).as_slice(),
        );
    }

    // Note that calling `root()` is mandatory to build the trie
    hb.root();

    // Extract the proof nodes from the trie
    let proof = hb
        .take_proof_nodes()
        .into_nodes_sorted()
        .into_iter()
        .map(|n| n.1)
        .collect::<Vec<_>>();

    proof
}

/// Given a receipt, the root hash, and a proof, verify the proof.
pub fn verify_receipt_proof<N: NetworkSpec>(
    receipt: &N::ReceiptResponse,
    receipts_len: usize,
    root: B256,
    proof: &[Bytes],
) -> bool {
    let index = receipt.transaction_index().unwrap() as usize;
    let index = adjust_index_for_rlp(index, receipts_len);
    let index_buffer = rlp::encode_fixed_size(&index);
    let key = Nibbles::unpack(&index_buffer);
    let expected_value = Some(N::encode_receipt(receipt));

    mpt_verify_proof(root, key, expected_value, proof).is_ok()
}
