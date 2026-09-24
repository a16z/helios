use alloy::primitives::B256;
use sha2::{Digest, Sha256};
use tree_hash::TreeHash;

use crate::{
    consensus_spec::ConsensusSpec,
    types::{BeaconBlockHeader, ExecutionPayloadHeader, Forks, SyncCommittee},
};

pub fn is_finality_proof_valid(
    attested_header: &BeaconBlockHeader,
    finality_header: Option<&BeaconBlockHeader>,
    finality_branch: &[B256],
    current_epoch: u64,
    forks: &Forks,
) -> bool {
    let (index, depth) = if current_epoch >= forks.gloas.epoch {
        (223, 9)
    } else if current_epoch >= forks.electra.epoch {
        (41, 7)
    } else {
        (41, 6)
    };

    is_root_proof_valid(
        attested_header.state_root,
        finality_header
            .map(TreeHash::tree_hash_root)
            .unwrap_or(B256::ZERO),
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
    let (index, depth) = if current_epoch >= forks.gloas.epoch {
        (898, 11)
    } else if current_epoch >= forks.electra.epoch {
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
    let (index, depth) = if current_epoch >= forks.gloas.epoch {
        (897, 11)
    } else if current_epoch >= forks.electra.epoch {
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

pub fn is_execution_block_hash_proof_valid(
    attested_header: &BeaconBlockHeader,
    execution_block_hash: B256,
    execution_branch: &[B256],
    epoch: u64,
    forks: &Forks,
) -> bool {
    let (index, depth) = if epoch >= forks.gloas.epoch {
        (808, 11)
    } else if epoch >= forks.deneb.epoch {
        (300, 9)
    } else if epoch >= forks.capella.epoch {
        (156, 8)
    } else {
        return execution_block_hash.is_zero() && execution_branch.iter().all(B256::is_zero);
    };

    is_root_proof_valid(
        attested_header.body_root,
        execution_block_hash,
        execution_branch,
        depth,
        index,
    )
}

fn is_proof_valid<T: TreeHash>(
    root: B256,
    leaf_object: &T,
    branch: &[B256],
    depth: usize,
    index: usize,
) -> bool {
    is_root_proof_valid(root, leaf_object.tree_hash_root(), branch, depth, index)
}

fn is_root_proof_valid(
    root: B256,
    leaf_root: B256,
    branch: &[B256],
    depth: usize,
    index: usize,
) -> bool {
    // Fork upgrades prepend zeroes to fit the newer fork's branch type.
    let Some(extra) = branch.len().checked_sub(depth) else {
        return false;
    };
    if branch[..extra].iter().any(|node| !node.is_zero()) {
        return false;
    }

    compute_merkle_root(leaf_root, &branch[extra..], index) == root
}

fn compute_merkle_root(leaf_root: B256, branch: &[B256], index: usize) -> B256 {
    let mut derived_root = leaf_root;
    let mut hasher = Sha256::new();

    for (i, node) in branch.iter().enumerate() {
        if !(index / 2usize.pow(i as u32)).is_multiple_of(2) {
            hasher.update(node);
            hasher.update(derived_root);
        } else {
            hasher.update(derived_root);
            hasher.update(node);
        }

        derived_root = B256::from_slice(hasher.finalize_reset().as_slice());
    }

    derived_root
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Fork;

    #[test]
    fn genesis_finality_still_requires_a_valid_proof() {
        let forks = Forks {
            gloas: Fork {
                epoch: 0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut branch = vec![B256::repeat_byte(1); 9];
        let header = BeaconBlockHeader {
            state_root: compute_merkle_root(B256::ZERO, &branch, 223),
            ..Default::default()
        };
        assert!(is_finality_proof_valid(&header, None, &branch, 0, &forks));
        branch[0] = B256::ZERO;
        assert!(!is_finality_proof_valid(&header, None, &branch, 0, &forks));
    }

    #[test]
    fn execution_proofs_follow_the_header_fork() {
        let forks = Forks {
            capella: Fork {
                epoch: 1,
                ..Default::default()
            },
            deneb: Fork {
                epoch: 2,
                ..Default::default()
            },
            gloas: Fork {
                epoch: 3,
                ..Default::default()
            },
            ..Default::default()
        };
        let leaf = B256::repeat_byte(1);
        for (epoch, depth, index) in [(1, 8, 156), (2, 9, 300), (3, 11, 808)] {
            let branch = vec![B256::repeat_byte(2); depth];
            let header = BeaconBlockHeader {
                body_root: compute_merkle_root(leaf, &branch, index),
                ..Default::default()
            };
            let mut normalized = vec![B256::ZERO; 11 - depth];
            normalized.extend(branch);
            assert!(is_execution_block_hash_proof_valid(
                &header,
                leaf,
                &normalized,
                epoch,
                &forks
            ));
            assert!(!is_execution_block_hash_proof_valid(
                &header,
                B256::ZERO,
                &normalized,
                epoch,
                &forks
            ));
            normalized[0] = B256::repeat_byte(3);
            assert!(!is_execution_block_hash_proof_valid(
                &header,
                leaf,
                &normalized,
                epoch,
                &forks
            ));
            assert!(!is_execution_block_hash_proof_valid(
                &header,
                leaf,
                &normalized[..depth - 1],
                epoch,
                &forks
            ));
        }
        assert!(is_execution_block_hash_proof_valid(
            &BeaconBlockHeader::default(),
            B256::ZERO,
            &[B256::ZERO; 11],
            0,
            &forks
        ));
        assert!(!is_execution_block_hash_proof_valid(
            &BeaconBlockHeader::default(),
            leaf,
            &[B256::ZERO; 11],
            0,
            &forks
        ));
    }
}
