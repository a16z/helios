//! Regression tests for genesis finality validation.
//!
//! At the genesis finalized checkpoint the specification requires the finalized header to be
//! empty and the finality branch to prove the zero root. These tests fabricate a minimal-preset
//! store and validly signed updates so the finality checks, not the signature check, decide the
//! outcome.

use alloy::primitives::B256;
use bls12_381::{
    hash_to_curve::{ExpandMsgXmd, HashToCurve},
    G1Affine, G1Projective, G2Affine, G2Projective, Scalar,
};
use helios_consensus_core::{
    consensus_spec::MinimalConsensusSpec,
    errors::ConsensusError,
    types::{
        bls::{PublicKey, Signature},
        BeaconBlockHeader, FinalityUpdate, FinalityUpdateBase, Fork, Forks, GenericUpdate,
        LightClientHeader, LightClientHeaderBellatrix, LightClientHeaderGloas, LightClientStore,
        SyncAggregate, SyncCommittee, Update, UpdateBase, UpdateGloas,
    },
    verify_finality_update, verify_generic_update, verify_update,
};
use sha2::{Digest, Sha256};
use ssz::Decode;
use ssz_types::{BitVector, FixedVector};
use tree_hash::TreeHash;

type S = MinimalConsensusSpec;

const SYNC_COMMITTEE_SIZE: usize = 32;
const CURRENT_SLOT: u64 = 82;
const FINALITY_INDEX: usize = 41;
const FINALITY_DEPTH: usize = 6;
const GLOAS_FINALITY_INDEX: usize = 223;
const GLOAS_FINALITY_DEPTH: usize = 9;
const GLOAS_EXECUTION_INDEX: usize = 808;
const GLOAS_EXECUTION_DEPTH: usize = 11;
const SECRET: u64 = SYNC_COMMITTEE_SIZE as u64;
const GARBAGE_BRANCH: [B256; FINALITY_DEPTH] = [B256::repeat_byte(0x77); FINALITY_DEPTH];
const ZERO_ROOT_BRANCH: [B256; FINALITY_DEPTH] = [B256::repeat_byte(0x42); FINALITY_DEPTH];
const HEADER_BRANCH: [B256; FINALITY_DEPTH] = [B256::repeat_byte(0x24); FINALITY_DEPTH];

fn forks() -> Forks {
    // Disable every fork from Capella on so headers use the Bellatrix layout and the finality
    // branch has depth 6.
    let disabled = || Fork {
        epoch: u64::MAX,
        ..Default::default()
    };
    Forks {
        capella: disabled(),
        deneb: disabled(),
        electra: disabled(),
        fulu: disabled(),
        gloas: disabled(),
        ..Default::default()
    }
}

fn gloas_forks() -> Forks {
    // Gloas headers carry an execution block hash proven against the body root, and the finality
    // branch grows to 9 nodes.
    Forks {
        gloas: Fork {
            epoch: 0,
            ..Default::default()
        },
        ..forks()
    }
}

fn public_key(bytes: &[u8; 48]) -> PublicKey {
    PublicKey::from_ssz_bytes(bytes).unwrap()
}

/// A committee whose members all use the same key. Its aggregate key and the fabricated
/// signature therefore use `SECRET * G1/G2`.
fn committee() -> SyncCommittee<S> {
    let member = public_key(&G1Affine::generator().to_compressed());
    let aggregate = G1Affine::from(G1Projective::generator() * Scalar::from(SECRET));
    SyncCommittee {
        pubkeys: FixedVector::new(vec![member; SYNC_COMMITTEE_SIZE]).unwrap(),
        aggregate_pubkey: public_key(&aggregate.to_compressed()),
    }
}

/// The signing root for the zero fork version and zero genesis validators root used by these
/// tests.
fn signing_root(attested: &BeaconBlockHeader) -> [u8; 32] {
    let fork_data_root = Sha256::digest(&[0u8; 64]);
    let mut domain = [0u8; 32];
    domain[0] = 7; // DOMAIN_SYNC_COMMITTEE
    domain[4..].copy_from_slice(&fork_data_root[..28]);

    let mut signing_data = [0u8; 64];
    signing_data[..32].copy_from_slice(attested.tree_hash_root().as_slice());
    signing_data[32..].copy_from_slice(&domain);
    Sha256::digest(&signing_data).into()
}

fn sign(attested: &BeaconBlockHeader) -> Signature {
    const DST: &[u8] = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_";
    let message = <G2Projective as HashToCurve<ExpandMsgXmd<sha2::Sha256>>>::hash_to_curve(
        signing_root(attested),
        DST,
    );
    let signature = G2Affine::from(message * Scalar::from(SECRET));
    Signature::from_ssz_bytes(&signature.to_compressed()).unwrap()
}

fn sync_aggregate(attested: &LightClientHeader) -> SyncAggregate<S> {
    let mut bits = BitVector::new();
    for index in 0..SYNC_COMMITTEE_SIZE {
        bits.set(index, true).unwrap();
    }

    SyncAggregate {
        sync_committee_bits: bits,
        sync_committee_signature: sign(attested.beacon()),
    }
}

fn header(slot: u64, state_root: B256) -> LightClientHeader {
    LightClientHeader::Bellatrix(LightClientHeaderBellatrix {
        beacon: BeaconBlockHeader {
            slot,
            state_root,
            ..Default::default()
        },
    })
}

/// A Gloas header whose (empty) execution block hash is proven against `body_root`. The fork
/// upgrade makes the execution proof an 11-node branch at index 808.
fn gloas_header(slot: u64, state_root: B256) -> LightClientHeader {
    let execution_branch = vec![B256::ZERO; GLOAS_EXECUTION_DEPTH];
    let body_root = compute_merkle_root(B256::ZERO, &execution_branch, GLOAS_EXECUTION_INDEX);
    LightClientHeader::Gloas(LightClientHeaderGloas {
        beacon: BeaconBlockHeader {
            slot,
            state_root,
            body_root,
            ..Default::default()
        },
        execution_block_hash: B256::ZERO,
        execution_branch: FixedVector::new(execution_branch).unwrap(),
    })
}

/// A store finalized at slot 64, the first slot of sync committee period 1.
fn store() -> LightClientStore<S> {
    LightClientStore {
        finalized_header: header(64, B256::ZERO),
        optimistic_header: header(64, B256::ZERO),
        current_sync_committee: committee(),
        ..Default::default()
    }
}

fn update(
    attested_header: LightClientHeader,
    finalized_header: LightClientHeader,
    finality_branch: [B256; FINALITY_DEPTH],
) -> Update<S> {
    let signature_slot = CURRENT_SLOT - 1;
    Update::Base(UpdateBase {
        sync_aggregate: sync_aggregate(&attested_header),
        attested_header,
        next_sync_committee: Default::default(),
        next_sync_committee_branch: Default::default(),
        finalized_header,
        finality_branch: FixedVector::new(finality_branch.to_vec()).unwrap(),
        signature_slot,
    })
}

fn gloas_update(
    attested_header: LightClientHeader,
    finalized_header: LightClientHeader,
    finality_branch: [B256; GLOAS_FINALITY_DEPTH],
) -> Update<S> {
    let LightClientHeader::Gloas(attested_header_gloas) = attested_header else {
        panic!("Gloas updates carry Gloas headers");
    };
    let LightClientHeader::Gloas(finalized_header_gloas) = finalized_header else {
        panic!("Gloas updates carry Gloas headers");
    };

    Update::Gloas(UpdateGloas {
        sync_aggregate: sync_aggregate(&LightClientHeader::Gloas(attested_header_gloas.clone())),
        attested_header_gloas,
        next_sync_committee: Default::default(),
        next_sync_committee_branch: Default::default(),
        finalized_header_gloas,
        finality_branch: FixedVector::new(finality_branch.to_vec()).unwrap(),
        signature_slot: CURRENT_SLOT - 1,
    })
}

fn finality_update(
    attested_header: LightClientHeader,
    finalized_header: LightClientHeader,
    finality_branch: [B256; FINALITY_DEPTH],
) -> FinalityUpdate<S> {
    FinalityUpdate::Base(FinalityUpdateBase {
        sync_aggregate: sync_aggregate(&attested_header),
        attested_header,
        finalized_header,
        finality_branch: FixedVector::new(finality_branch.to_vec()).unwrap(),
        signature_slot: CURRENT_SLOT - 1,
    })
}

/// Mirrors the proof module's Merkle branch evaluation so tests can construct a state root that
/// commits to a known leaf.
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

fn zero_root_state_root() -> B256 {
    compute_merkle_root(B256::ZERO, &ZERO_ROOT_BRANCH, FINALITY_INDEX)
}

fn assert_invalid_finality(error: eyre::Report) {
    assert!(
        matches!(
            error.downcast_ref::<ConsensusError>(),
            Some(ConsensusError::InvalidFinalityProof)
        ),
        "{error:?}"
    );
}

fn invalid_finality_error(update: &Update<S>) -> eyre::Report {
    verify_update(update, CURRENT_SLOT, &store(), B256::ZERO, &forks()).unwrap_err()
}

#[test]
fn empty_genesis_header_with_invalid_finality_branch_is_rejected() {
    let update = update(
        header(80, B256::repeat_byte(0x11)),
        LightClientHeader::default(),
        GARBAGE_BRANCH,
    );

    assert_invalid_finality(invalid_finality_error(&update));
}

#[test]
fn empty_genesis_header_with_valid_zero_root_branch_is_accepted() {
    let update = update(
        header(80, zero_root_state_root()),
        LightClientHeader::default(),
        ZERO_ROOT_BRANCH,
    );

    verify_update(&update, CURRENT_SLOT, &store(), B256::ZERO, &forks())
        .expect("a valid genesis finality proof must be accepted");
}

#[test]
fn non_empty_genesis_header_with_valid_zero_root_branch_is_rejected() {
    // The branch is valid for the zero root, but the slot-zero header is not empty.
    let update = update(
        header(80, zero_root_state_root()),
        header(0, B256::repeat_byte(0x99)),
        ZERO_ROOT_BRANCH,
    );

    assert_invalid_finality(invalid_finality_error(&update));
}

#[test]
fn non_genesis_finality_header_with_garbage_branch_is_rejected() {
    // Control: only the genesis path relaxes the leaf to the zero root.
    let update = update(
        header(80, B256::repeat_byte(0x11)),
        header(72, B256::repeat_byte(0x22)),
        GARBAGE_BRANCH,
    );

    assert_invalid_finality(invalid_finality_error(&update));
}

#[test]
fn non_genesis_finality_update_with_valid_branch_is_accepted() {
    let finalized_header = header(72, B256::repeat_byte(0x22));
    let attested_state_root = compute_merkle_root(
        finalized_header.beacon().tree_hash_root(),
        &HEADER_BRANCH,
        FINALITY_INDEX,
    );
    let update = update(
        header(80, attested_state_root),
        finalized_header,
        HEADER_BRANCH,
    );

    verify_update(&update, CURRENT_SLOT, &store(), B256::ZERO, &forks())
        .expect("a normal finality update must be accepted");
}

#[test]
fn update_without_finality_branch_is_accepted() {
    let update = update(
        header(80, B256::repeat_byte(0x11)),
        LightClientHeader::default(),
        [B256::ZERO; FINALITY_DEPTH],
    );

    verify_update(&update, CURRENT_SLOT, &store(), B256::ZERO, &forks())
        .expect("an update without a finality branch must be accepted");
}

#[test]
fn concrete_genesis_header_without_finality_branch_keeps_existing_tolerance() {
    // Some clients return the concrete genesis header instead of the empty one when there is no
    // finality branch. Preserve the existing compatibility behavior for that case.
    let update = update(
        header(80, B256::repeat_byte(0x11)),
        header(0, B256::repeat_byte(0x99)),
        [B256::ZERO; FINALITY_DEPTH],
    );

    verify_update(&update, CURRENT_SLOT, &store(), B256::ZERO, &forks())
        .expect("the existing genesis header tolerance must be preserved");
}

#[test]
fn finality_update_with_invalid_genesis_branch_is_rejected() {
    let update = finality_update(
        header(80, B256::repeat_byte(0x11)),
        LightClientHeader::default(),
        GARBAGE_BRANCH,
    );

    let error =
        verify_finality_update(&update, CURRENT_SLOT, &store(), B256::ZERO, &forks()).unwrap_err();
    assert_invalid_finality(error);
}

#[test]
fn finality_update_with_valid_zero_root_branch_is_accepted() {
    let update = finality_update(
        header(80, zero_root_state_root()),
        LightClientHeader::default(),
        ZERO_ROOT_BRANCH,
    );

    verify_finality_update(&update, CURRENT_SLOT, &store(), B256::ZERO, &forks())
        .expect("a valid genesis finality update must be accepted");
}

#[test]
fn gloas_empty_genesis_header_with_valid_zero_root_branch_is_accepted() {
    let branch = [B256::repeat_byte(0x42); GLOAS_FINALITY_DEPTH];
    let attested_state_root = compute_merkle_root(B256::ZERO, &branch, GLOAS_FINALITY_INDEX);
    let update = gloas_update(
        gloas_header(80, attested_state_root),
        LightClientHeader::Gloas(LightClientHeaderGloas::default()),
        branch,
    );

    verify_update(&update, CURRENT_SLOT, &store(), B256::ZERO, &gloas_forks())
        .expect("a valid Gloas genesis finality proof must be accepted");
}

#[test]
fn gloas_genesis_header_with_execution_block_hash_is_rejected() {
    // The branch proves the zero root, but an empty Gloas header has a zero execution hash.
    let branch = [B256::repeat_byte(0x42); GLOAS_FINALITY_DEPTH];
    let attested_state_root = compute_merkle_root(B256::ZERO, &branch, GLOAS_FINALITY_INDEX);
    let finalized_header = LightClientHeaderGloas {
        execution_block_hash: B256::repeat_byte(0x01),
        ..Default::default()
    };
    let update = gloas_update(
        gloas_header(80, attested_state_root),
        LightClientHeader::Gloas(finalized_header),
        branch,
    );

    let error =
        verify_update(&update, CURRENT_SLOT, &store(), B256::ZERO, &gloas_forks()).unwrap_err();
    assert_invalid_finality(error);
}

#[test]
fn gloas_genesis_header_with_execution_branch_is_rejected() {
    // The branch proves the zero root, but an empty Gloas header has a zero execution branch.
    let branch = [B256::repeat_byte(0x42); GLOAS_FINALITY_DEPTH];
    let attested_state_root = compute_merkle_root(B256::ZERO, &branch, GLOAS_FINALITY_INDEX);
    let mut execution_branch = vec![B256::ZERO; GLOAS_EXECUTION_DEPTH];
    execution_branch[0] = B256::repeat_byte(0x01);
    let finalized_header = LightClientHeaderGloas {
        execution_branch: FixedVector::new(execution_branch).unwrap(),
        ..Default::default()
    };
    let update = gloas_update(
        gloas_header(80, attested_state_root),
        LightClientHeader::Gloas(finalized_header),
        branch,
    );

    let error =
        verify_update(&update, CURRENT_SLOT, &store(), B256::ZERO, &gloas_forks()).unwrap_err();
    assert_invalid_finality(error);
}

#[test]
fn generic_update_with_empty_finalized_header_and_no_branch_is_accepted() {
    // `GenericUpdate::from` normalizes an empty header to `None`, so this arm is only reachable
    // when a `GenericUpdate` is built directly.
    let attested_header = header(80, B256::repeat_byte(0x11));
    let update = GenericUpdate::<S> {
        sync_aggregate: sync_aggregate(&attested_header),
        attested_header,
        finalized_header: Some(LightClientHeader::default()),
        finality_branch: None,
        signature_slot: CURRENT_SLOT - 1,
        ..Default::default()
    };

    verify_generic_update(&update, CURRENT_SLOT, &store(), B256::ZERO, &forks())
        .expect("an empty finalized header without a branch is valid");
}

#[test]
fn generic_update_with_empty_finalized_header_and_invalid_branch_is_rejected() {
    let attested_header = header(80, B256::repeat_byte(0x11));
    let update = GenericUpdate::<S> {
        sync_aggregate: sync_aggregate(&attested_header),
        attested_header,
        finalized_header: Some(LightClientHeader::default()),
        finality_branch: Some(GARBAGE_BRANCH.to_vec()),
        signature_slot: CURRENT_SLOT - 1,
        ..Default::default()
    };

    let error =
        verify_generic_update(&update, CURRENT_SLOT, &store(), B256::ZERO, &forks()).unwrap_err();
    assert_invalid_finality(error);
}
