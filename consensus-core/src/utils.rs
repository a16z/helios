use alloy::primitives::B256;
use milagro_bls::{AggregateSignature, PublicKey};
use sha2::{Digest, Sha256};
use ssz_types::FixedVector;
use tree_hash::TreeHash;
use tree_hash_derive::TreeHash;

use crate::types::{Header, SignatureBytes};

pub fn calc_sync_period(slot: u64) -> u64 {
    // 32 slots per epoch
    let epoch = slot / 32;
    // 256 epochs per sync committee
    epoch / 256
}

pub fn is_aggregate_valid(sig_bytes: &SignatureBytes, msg: &[u8], pks: &[&PublicKey]) -> bool {
    let sig_res = AggregateSignature::from_bytes(&sig_bytes.inner);

    match sig_res {
        Ok(sig) => sig.fast_aggregate_verify(msg, pks),
        Err(_) => false,
    }
}

pub fn is_proof_valid<L: TreeHash>(
    attested_header: &Header,
    leaf_object: &mut L,
    branch: &[B256],
    depth: usize,
    index: usize,
) -> bool {
    if branch.len() != depth {
        return false;
    }

    let mut derived_root = leaf_object.tree_hash_root();
    let mut hasher = Sha256::new();

    for (i, node) in branch.iter().enumerate() {
        if (index / 2usize.pow(i as u32)) % 2 != 0 {
            hasher.update(node);
            hasher.update(derived_root);
        } else {
            hasher.update(derived_root);
            hasher.update(node);
        }

        derived_root = B256::from_slice(&hasher.finalize_reset());
    }

    derived_root == attested_header.state_root
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

pub fn compute_signing_root(object_root: B256, domain: B256) -> B256 {
    let data = SigningData {
        object_root,
        domain,
    };

    data.tree_hash_root()
}

pub fn compute_domain(domain_type: &[u8; 4], fork_data_root: B256) -> B256 {
    let start = domain_type;
    let end = &fork_data_root[..28];
    let d = [start, end].concat();
    B256::from_slice(d.as_slice())
}

pub fn compute_fork_data_root(
    current_version: FixedVector<u8, typenum::U4>,
    genesis_validator_root: B256,
) -> B256 {
    let fork_data = ForkData {
        current_version,
        genesis_validator_root,
    };

    fork_data.tree_hash_root()
}
