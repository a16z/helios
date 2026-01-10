use alloy::primitives::B256;
use eyre::Result;
use ssz_types::{BitVector, FixedVector};
use tree_hash::TreeHash;
use tree_hash_derive::TreeHash;

use crate::{
    consensus_spec::ConsensusSpec,
    types::{Forks, SyncCommittee},
};
use bls12_381::{G1Affine, G1Projective};

pub fn compute_committee_sign_root(header: B256, fork_data_root: B256) -> B256 {
    let domain_type = [7, 00, 00, 00];
    let domain = compute_domain(domain_type, fork_data_root);
    compute_signing_root(header, domain)
}

pub fn calculate_fork_version<S: ConsensusSpec>(
    forks: &Forks,
    slot: u64,
) -> FixedVector<u8, typenum::U4> {
    let epoch = slot / S::slots_per_epoch();

    let version = if epoch >= forks.fulu.epoch {
        forks.fulu.fork_version
    } else if epoch >= forks.electra.epoch {
        forks.electra.fork_version
    } else if epoch >= forks.deneb.epoch {
        forks.deneb.fork_version
    } else if epoch >= forks.capella.epoch {
        forks.capella.fork_version
    } else if epoch >= forks.bellatrix.epoch {
        forks.bellatrix.fork_version
    } else if epoch >= forks.altair.epoch {
        forks.altair.fork_version
    } else {
        forks.genesis.fork_version
    };

    FixedVector::from(version.as_slice().to_vec())
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

/// Computes the aggregate public key for the participating members of the sync committee.
/// with given participation bitfield and committee aggregate public key.
pub fn get_participating_aggregate_pubkey<S: ConsensusSpec>(
    committee: &SyncCommittee<S>,
    bitfield: &BitVector<S::SyncCommitteeSize>,
) -> Result<G1Affine> {
    let total = bitfield.len();
    let participating = bitfield.iter().filter(|b| *b).count();
    if participating == 0 {
        return Err(eyre::eyre!("no participating keys"));
    }

    if participating > total / 2 {
        let mut agg = G1Projective::from(committee.aggregate_pubkey.point()?);
        for (i, bit) in bitfield.iter().enumerate() {
            if !bit {
                agg -= G1Projective::from(committee.pubkeys[i].point()?);
            }
        }
        Ok(G1Affine::from(agg))
    } else {
        let mut agg = G1Projective::identity();
        for (i, bit) in bitfield.iter().enumerate() {
            if bit {
                agg += G1Projective::from(committee.pubkeys[i].point()?);
            }
        }
        Ok(G1Affine::from(agg))
    }
}

fn compute_signing_root(object_root: B256, domain: B256) -> B256 {
    let data = SigningData {
        object_root,
        domain,
    };

    data.tree_hash_root()
}

fn compute_domain(domain_type: [u8; 4], fork_data_root: B256) -> B256 {
    let start = &domain_type;
    let end = &fork_data_root[..28];
    let d = [start, end].concat();
    B256::from_slice(d.as_slice())
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
