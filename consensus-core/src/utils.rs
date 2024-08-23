use alloy::primitives::B256;
use eyre::Result;
use milagro_bls::{AggregateSignature, PublicKey};
use ssz_types::FixedVector;
use tree_hash_derive::TreeHash;
use tree_hash::TreeHash;

use crate::types::{Header, SignatureBytes};

pub fn calc_sync_period(slot: u64) -> u64 {
    // 32 slots per epoch
    let epoch = slot / 32;
    // 256 epochs per sync committee
    epoch / 256 
}

pub fn is_aggregate_valid(sig_bytes: &SignatureBytes, msg: &[u8], pks: &[&PublicKey]) -> bool {
    let sig_res = AggregateSignature::from_bytes(sig_bytes);

    match sig_res {
        Ok(sig) => sig.fast_aggregate_verify(msg, pks),
        Err(_) => false,
    }
}

pub fn is_proof_valid<L>(
    attested_header: &Header,
    leaf_object: &mut L,
    branch: &[B256],
    depth: usize,
    index: usize,
) -> bool {
    true
    // let res: Result<bool> = (move || {
    //     let leaf_hash = leaf_object.hash_tree_root()?;
    //     let state_root = bytes32_to_node(&attested_header.state_root)?;
    //     let branch = branch_to_nodes(branch.to_vec())?;

    //     let is_valid = is_valid_merkle_branch(&leaf_hash, branch.iter(), depth, index, &state_root);
    //     Ok(is_valid)
    // })();

    // if let Ok(is_valid) = res {
    //     is_valid
    // } else {
    //     false
    // }
}

#[derive(Default, Debug, TreeHash)]
struct SigningData {
    object_root: B256,
    domain: B256,
}

#[derive(Default, Debug, TreeHash)]
struct ForkData {
    current_version: FixedVector<u8, typenum::U4>,
    genesis_validator_root: B256,
}

pub fn compute_signing_root(object_root: B256, domain: B256) -> Result<B256> {
    let data = SigningData {
        object_root,
        domain,
    };

    Ok(data.tree_hash_root())
}

pub fn compute_domain(domain_type: &[u8], fork_data_root: B256) -> Result<B256> {
    let start = domain_type;
    let end = &fork_data_root[..28];
    let d = [start, end].concat();
    Ok(B256::from_slice(d.as_slice()))
}

pub fn compute_fork_data_root(
    current_version: FixedVector<u8, typenum::U4>,
    genesis_validator_root: B256,
) -> Result<B256> {
    let fork_data = ForkData {
        current_version,
        genesis_validator_root,
    };

    Ok(fork_data.tree_hash_root())
}

// pub fn branch_to_nodes(branch: Vec<B256>) -> Result<Vec<Node>> {
//     branch
//         .iter()
//         .map(bytes32_to_node)
//         .collect::<Result<Vec<Node>>>()
// }
// 
// pub fn bytes32_to_node(bytes: &Bytes32) -> Result<Node> {
//     Ok(Node::try_from(bytes.as_slice())?)
// }
