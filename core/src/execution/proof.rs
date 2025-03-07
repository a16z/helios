use alloy::consensus::BlockHeader;
use alloy::network::{BlockResponse, ReceiptResponse};
use alloy::primitives::{keccak256, Bytes, B256, U256};
use alloy::rlp;
use alloy::rpc::types::EIP1186AccountProofResponse;
use alloy_trie::root::ordered_trie_root_with_encoder;
use alloy_trie::{
    proof::{verify_proof, ProofRetainer},
    root::adjust_index_for_rlp,
    HashBuilder, Nibbles, TrieAccount, KECCAK_EMPTY,
};
use eyre::{eyre, Result};

use helios_common::{network_spec::NetworkSpec, types::BlockTag};

use super::errors::ExecutionError;

/// Verify a given `EIP1186AccountProofResponse`'s account proof against given state root.
pub fn verify_account_proof(proof: &EIP1186AccountProofResponse, state_root: B256) -> Result<()> {
    let account_key = proof.address;
    let account = TrieAccount {
        nonce: proof.nonce,
        balance: proof.balance,
        storage_root: proof.storage_hash,
        code_hash: proof.code_hash,
    };

    verify_mpt_proof(state_root, account_key, account, &proof.account_proof)
        .map_err(|_| eyre!(ExecutionError::InvalidAccountProof(proof.address)))
}

/// Verify a given `EIP1186AccountProofResponse`'s storage proof against the storage root.
pub fn verify_storage_proof(proof: &EIP1186AccountProofResponse) -> Result<()> {
    for storage_proof in &proof.storage_proof {
        let key = storage_proof.key.as_b256();
        let value = storage_proof.value;

        verify_mpt_proof(proof.storage_hash, key, value, &storage_proof.proof)
            .map_err(|_| ExecutionError::InvalidStorageProof(proof.address, key))?;
    }

    Ok(())
}

/// Verify a given `EIP1186AccountProofResponse`'s code hash against the given code.
pub fn verify_code_hash_proof(proof: &EIP1186AccountProofResponse, code: &Bytes) -> Result<()> {
    if (proof.code_hash == KECCAK_EMPTY || proof.code_hash == B256::ZERO) && code.is_empty() {
        Ok(())
    } else {
        let code_hash = keccak256(code);
        if proof.code_hash != code_hash {
            return Err(ExecutionError::CodeHashMismatch(
                proof.address,
                code_hash,
                proof.code_hash,
            )
            .into());
        }
        Ok(())
    }
}

/// Verifies a MPT proof for a given key-value pair against the provided root hash.
/// This function wraps `alloy_trie::proof::verify_proof` and checks
/// if the value represents an empty account or slot to support exclusion proofs.
///
/// # Parameters
/// - `root`: The root hash of the MPT.
/// - `raw_key`: The key to be verified, which will be hashed using `keccak256`.
/// - `raw_value`: The value associated with the key, which will be RLP encoded.
/// - `proof`: A slice of bytes representing the MPT proof.
pub fn verify_mpt_proof<K: AsRef<[u8]>, V: rlp::Encodable>(
    root: B256,
    raw_key: K,
    raw_value: V,
    proof: &[Bytes],
) -> Result<()> {
    let key = Nibbles::unpack(keccak256(raw_key));
    let value = rlp::encode(raw_value);

    let value = if is_empty_value(&value) {
        None // exclusion proof
    } else {
        Some(value) // inclusion proof
    };

    verify_proof(root, key, value, proof).map_err(|e| eyre!(e))
}

/// Check if the value is an empty account or empty slot.
fn is_empty_value(value: &[u8]) -> bool {
    let empty_account = TrieAccount::default();
    let new_empty_account = TrieAccount {
        nonce: 0,
        balance: U256::ZERO,
        storage_root: B256::ZERO,
        code_hash: B256::ZERO,
    };

    let empty_account = rlp::encode(empty_account);
    let new_empty_account = rlp::encode(new_empty_account);

    let is_empty_slot = value.len() == 1 && value[0] == 0x80;
    let is_empty_account = value == empty_account || value == new_empty_account;
    is_empty_slot || is_empty_account
}

/// Create a MPT proof for a given receipt in a list of receipts.
pub fn create_receipt_proof<N: NetworkSpec>(
    receipts: Vec<N::ReceiptResponse>,
    target_index: usize,
) -> Vec<Bytes> {
    // Initialise the trie builder with proof retainer for the target index
    let receipts_len = receipts.len();
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
    hb.take_proof_nodes()
        .into_nodes_sorted()
        .into_iter()
        .map(|n| n.1)
        .collect::<Vec<_>>()
}

/// Given a receipt, the root hash, and a proof, verify the proof.
pub fn verify_receipt_proof<N: NetworkSpec>(
    receipt: &N::ReceiptResponse,
    root: B256,
    proof: &[Bytes],
) -> Result<()> {
    let key = {
        let index = receipt.transaction_index().unwrap() as usize;
        let index_buffer = rlp::encode_fixed_size(&index);
        Nibbles::unpack(&index_buffer)
    };
    let expected_value = Some(N::encode_receipt(receipt));

    verify_proof(root, key, expected_value, proof).map_err(|e| eyre!(e))
}

/// Calculate the receipts root hash from given list of receipts
/// and verify it against the given block's receipts root.
pub fn verify_block_receipts<N: NetworkSpec>(
    receipts: &[N::ReceiptResponse],
    block: &N::BlockResponse,
) -> Result<()> {
    let receipts_encoded = receipts.iter().map(N::encode_receipt).collect::<Vec<_>>();
    let expected_receipt_root = ordered_trie_root_noop_encoder(&receipts_encoded);

    if expected_receipt_root != block.header().receipts_root() {
        return Err(ExecutionError::BlockReceiptsRootMismatch(BlockTag::Number(
            block.header().number(),
        ))
        .into());
    }

    Ok(())
}

/// Compute a trie root of a collection of encoded items.
/// Ref: https://github.com/alloy-rs/trie/blob/main/src/root.rs.
pub fn ordered_trie_root_noop_encoder(items: &[Vec<u8>]) -> B256 {
    #[allow(clippy::ptr_arg)] // the fn signature is fixed in the external crate
    fn noop_encoder(item: &Vec<u8>, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(item);
    }

    ordered_trie_root_with_encoder(items, noop_encoder)
}

#[cfg(test)]
mod tests {
    use alloy::primitives::b256;

    use helios_ethereum::spec::Ethereum as EthereumSpec;
    use helios_test_utils::*;

    use super::*;

    #[test]
    fn test_verify_account_proof() {
        let proof = rpc_proof();
        let state_root = rpc_block().header().state_root();

        let result = verify_account_proof(&proof, state_root);

        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_storage_proof() {
        let proof = rpc_proof();

        let result = verify_storage_proof(&proof);

        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_code_hash_proof() {
        let proof = rpc_proof();
        let code = rpc_account().code.unwrap();

        let result = verify_code_hash_proof(&proof, &code);

        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_code_hash_proof_empty_hash() {
        let proof = EIP1186AccountProofResponse::default();
        let code = Bytes::new();

        let result = verify_code_hash_proof(&proof, &code);

        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_code_hash_proof_empty_keccak() {
        let mut proof = EIP1186AccountProofResponse::default();
        proof.code_hash = KECCAK_EMPTY;
        let code = Bytes::new();

        let result = verify_code_hash_proof(&proof, &code);

        assert!(result.is_ok());
    }

    #[test]
    fn test_create_receipt_proof() {
        let receipts = rpc_block_receipts();
        let expected = verifiable_api_tx_receipt_response();

        let proof = create_receipt_proof::<EthereumSpec>(
            receipts,
            expected.receipt.transaction_index().unwrap() as usize,
        );

        assert_eq!(proof, expected.receipt_proof);
    }

    #[test]
    fn test_verify_receipt_proof() {
        let receipts = rpc_block_receipts();
        let receipts_root = rpc_block().header().receipts_root();

        for idx in 0..receipts.len() {
            let proof = create_receipt_proof::<EthereumSpec>(receipts.clone(), idx);

            let result =
                verify_receipt_proof::<EthereumSpec>(&receipts[idx], receipts_root, &proof);

            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_verify_block_receipts() {
        let receipts = rpc_block_receipts();
        let block = rpc_block();

        let result = verify_block_receipts::<EthereumSpec>(&receipts, &block);

        assert!(result.is_ok());
    }

    #[test]
    fn test_ordered_trie_root_noop_encoder() {
        let items = vec![vec![0u8; 32], vec![1u8; 32]];

        let root = ordered_trie_root_noop_encoder(&items);

        assert_eq!(
            root,
            b256!("0x5369c64e2b262259b880114e7ac092e69c673e2671352991989f3a89fbc8f0af")
        );
    }
}
