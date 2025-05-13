use alloy::primitives::B256;
use sha2::{Digest, Sha256};
use tree_hash::TreeHash;

use crate::{
    consensus_spec::ConsensusSpec,
    types::{BeaconBlockHeader, ExecutionPayloadHeader, Forks, SyncCommittee},
};

pub fn is_finality_proof_valid(
    attested_header: &BeaconBlockHeader,
    finality_header: &BeaconBlockHeader,
    finality_branch: &[B256],
    current_epoch: u64,
    forks: &Forks,
) -> bool {
    let (index, depth) = if current_epoch >= forks.electra.epoch {
        (41, 7)
    } else {
        (41, 6)
    };

    is_proof_valid(
        attested_header.state_root,
        finality_header,
        finality_branch,
        depth,
        index,
    )
}

pub fn is_next_committee_proof_valid<S: ConsensusSpec>(
    attested_header: &BeaconBlockHeader,
    next_committee: &SyncCommittee<S>,
    next_committee_branch: &[B256],
    current_epoch: u64,
    forks: &Forks,
) -> bool {
    let (index, depth) = if current_epoch >= forks.electra.epoch {
        (23, 6)
    } else {
        (23, 5)
    };

    is_proof_valid(
        attested_header.state_root,
        next_committee,
        next_committee_branch,
        depth,
        index,
    )
}

pub fn is_current_committee_proof_valid<S: ConsensusSpec>(
    attested_header: &BeaconBlockHeader,
    current_committee: &SyncCommittee<S>,
    current_committee_branch: &[B256],
    current_epoch: u64,
    forks: &Forks,
) -> bool {
    let (index, depth) = if current_epoch >= forks.electra.epoch {
        (22, 6)
    } else {
        (22, 5)
    };

    is_proof_valid(
        attested_header.state_root,
        current_committee,
        current_committee_branch,
        depth,
        index,
    )
}

pub fn is_execution_payload_proof_valid(
    attested_header: &BeaconBlockHeader,
    execution: &ExecutionPayloadHeader,
    execution_branch: &[B256],
) -> bool {
    is_proof_valid(attested_header.body_root, execution, execution_branch, 4, 9)
}

fn is_proof_valid<T: TreeHash>(
    root: B256,
    leaf_object: &T,
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

        derived_root = B256::from_slice(hasher.finalize_reset().as_slice());
    }

    derived_root == root
}
