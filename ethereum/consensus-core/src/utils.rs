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

    let version = if epoch >= forks.gloas.epoch {
        forks.gloas.fork_version
    } else if epoch >= forks.fulu.epoch {
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

    // FastAggregateVerify runs KeyValidate on every participant's key, which rejects the
    // identity. Check the bytes so the more-than-half path still decodes no participant key.
    let has_identity = bitfield
        .iter()
        .zip(committee.pubkeys.iter())
        .any(|(bit, pubkey)| bit && pubkey.is_identity());
    if has_identity {
        return Err(eyre::eyre!("participating key is the identity"));
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

#[cfg(test)]
mod tests {
    use alloy::primitives::hex;
    use bls12_381::{G1Affine, G1Projective};
    use ssz::Decode;
    use ssz_types::BitVector;

    use super::get_participating_aggregate_pubkey;
    use crate::{
        consensus_spec::MinimalConsensusSpec,
        types::{
            bls::{PublicKey, Signature},
            SyncCommittee,
        },
    };

    // Official vector `eth_fast_aggregate_verify_infinity_pubkey` from
    // tests/general/altair/bls/eth_fast_aggregate_verify/bls/ in consensus-specs
    // v1.7.0-beta.4 general.tar.gz. Its expected output is `false`: the last key is
    // the identity, which KeyValidate rejects.
    const VECTOR_PUBKEYS: [&str; 4] = [
        "a491d1b0ecd9bb917989f0e74f0dea0422eac4a873e5e2644f368dffb9a6e20fd6e10c1b77654d067c0618f6e5a7f79a",
        "b301803f8b5ac4a1133581fc676dfedc60d891dd5fa99028805e5ea5b08d3491af75d0707adab3b70c6a6a580217bf81",
        "b53d21a4cfd562c469cc81514d4ce5a6b577d8403d32a394dc265dd190b47fa9f829fdd7963afdf972e5e77854051f6f",
        "c00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
    ];
    const VECTOR_MESSAGE: &str = "1212121212121212121212121212121212121212121212121212121212121212";
    const VECTOR_SIGNATURE: &str = "afcb4d980f079265caa61aee3e26bf48bebc5dc3e7f2d7346834d76cbc812f636c937b6b44a9323d8bc4b1cdf71d6811035ddc2634017faab2845308f568f2b9a0356140727356eae9eded8b87fd8cb8024b440c57aee06076128bb32921f584";
    const IDENTITY: usize = 3;

    fn pubkey(hex_str: &str) -> PublicKey {
        PublicKey::from_ssz_bytes(&hex::decode(hex_str).unwrap()).unwrap()
    }

    fn aggregate(keys: &[PublicKey]) -> PublicKey {
        let sum = keys
            .iter()
            .fold(G1Projective::identity(), |acc, key: &PublicKey| {
                acc + key.point().unwrap()
            });
        PublicKey::from_ssz_bytes(&G1Affine::from(sum).to_compressed()).unwrap()
    }

    /// Runs FastAggregateVerify for the vector through the light client's aggregation.
    fn fast_aggregate_verify(
        committee: &SyncCommittee<MinimalConsensusSpec>,
        bits: &BitVector<typenum::U32>,
    ) -> bool {
        let signature = Signature::from_ssz_bytes(&hex::decode(VECTOR_SIGNATURE).unwrap()).unwrap();
        let message = hex::decode(VECTOR_MESSAGE).unwrap();
        get_participating_aggregate_pubkey(committee, bits)
            .map(|pk| signature.verify(&message, &pk))
            .unwrap_or(false)
    }

    #[test]
    fn infinity_pubkey_vector_at_most_half_participating() {
        let vector: Vec<PublicKey> = VECTOR_PUBKEYS.iter().map(|k| pubkey(k)).collect();
        // Fill the rest of the committee with a valid key that does not participate.
        let mut pubkeys = vec![vector[0].clone(); 32];
        pubkeys[..4].clone_from_slice(&vector);
        let committee = SyncCommittee::<MinimalConsensusSpec> {
            aggregate_pubkey: aggregate(&pubkeys),
            pubkeys: pubkeys.into(),
        };
        let mut bits = BitVector::<typenum::U32>::new();
        for i in 0..4 {
            bits.set(i, true).unwrap();
        }

        assert!(!fast_aggregate_verify(&committee, &bits));

        // Control: the same signature without the identity key is valid.
        bits.set(IDENTITY, false).unwrap();
        assert!(fast_aggregate_verify(&committee, &bits));
    }

    #[test]
    fn infinity_pubkey_vector_more_than_half_participating() {
        let vector: Vec<PublicKey> = VECTOR_PUBKEYS.iter().map(|k| pubkey(k)).collect();
        // Every member except the first three has the identity key and participates.
        let mut pubkeys = vec![vector[IDENTITY].clone(); 32];
        pubkeys[..3].clone_from_slice(&vector[..3]);
        let committee = SyncCommittee::<MinimalConsensusSpec> {
            aggregate_pubkey: aggregate(&pubkeys),
            pubkeys: pubkeys.into(),
        };
        let mut bits = BitVector::<typenum::U32>::new();
        for i in 0..32 {
            bits.set(i, true).unwrap();
        }

        assert!(!fast_aggregate_verify(&committee, &bits));
    }
}
