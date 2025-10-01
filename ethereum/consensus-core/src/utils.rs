use alloy::primitives::B256;
use eyre::Result;
use ssz_types::{BitVector, FixedVector};
use tree_hash::TreeHash;
use tree_hash_derive::TreeHash;

use crate::{
    consensus_spec::ConsensusSpec,
    types::{bls::PublicKey, Forks, SyncCommittee},
};

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

pub fn get_participating_keys<S: ConsensusSpec>(
    committee: &SyncCommittee<S>,
    bitfield: &BitVector<S::SyncCommitteeSize>,
) -> Result<Vec<PublicKey>> {
    let mut pks: Vec<PublicKey> = Vec::new();

    bitfield.iter().enumerate().for_each(|(i, bit)| {
        if bit {
            let pk = committee.pubkeys[i].clone();
            pks.push(pk);
        }
    });

    Ok(pks)
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
