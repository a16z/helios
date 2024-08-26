use std::cmp;

use alloy::primitives::B256;
use eyre::Result;
use ssz_types::{BitVector, FixedVector};
use tracing::{info, warn};
use tree_hash::TreeHash;
use zduny_wasm_timer::{SystemTime, UNIX_EPOCH};

use common::config::types::Forks;

use crate::errors::ConsensusError;
use crate::types::bls::{PublicKey, Signature};
use crate::types::{
    FinalityUpdate, GenericUpdate, Header, LightClientStore, OptimisticUpdate, SyncCommittee,
    Update,
};
use crate::utils::{
    calc_sync_period, compute_domain, compute_fork_data_root, compute_signing_root, is_proof_valid,
};

pub fn get_participating_keys(
    committee: &SyncCommittee,
    bitfield: &BitVector<typenum::U512>,
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

pub fn get_bits(bitfield: &BitVector<typenum::U512>) -> u64 {
    bitfield.iter().filter(|v| *v).count() as u64
}

pub fn is_finality_proof_valid(
    attested_header: &Header,
    finality_header: &Header,
    finality_branch: &[B256],
) -> bool {
    is_proof_valid(attested_header, finality_header, finality_branch, 6, 41)
}

pub fn is_next_committee_proof_valid(
    attested_header: &Header,
    next_committee: &SyncCommittee,
    next_committee_branch: &[B256],
) -> bool {
    is_proof_valid(
        attested_header,
        next_committee,
        next_committee_branch,
        5,
        23,
    )
}

pub fn is_current_committee_proof_valid(
    attested_header: &Header,
    current_committee: &SyncCommittee,
    current_committee_branch: &[B256],
) -> bool {
    is_proof_valid(
        attested_header,
        current_committee,
        current_committee_branch,
        5,
        22,
    )
}

pub fn safety_threshold(store: &LightClientStore) -> u64 {
    cmp::max(
        store.current_max_active_participants,
        store.previous_max_active_participants,
    ) / 2
}

pub fn has_sync_update(update: &GenericUpdate) -> bool {
    update.next_sync_committee.is_some() && update.next_sync_committee_branch.is_some()
}

pub fn has_finality_update(update: &GenericUpdate) -> bool {
    update.finalized_header.is_some() && update.finality_branch.is_some()
}

// implements state changes from apply_light_client_update and process_light_client_update in
// the specification
/// Returns the new checkpoint if one is created, otherwise None
pub fn apply_generic_update(store: &mut LightClientStore, update: &GenericUpdate) -> Option<B256> {
    let committee_bits = get_bits(&update.sync_aggregate.sync_committee_bits);

    store.current_max_active_participants =
        u64::max(store.current_max_active_participants, committee_bits);

    let should_update_optimistic = committee_bits > safety_threshold(store)
        && update.attested_header.slot > store.optimistic_header.slot;

    if should_update_optimistic {
        store.optimistic_header = update.attested_header.clone();
    }

    let update_attested_period = calc_sync_period(update.attested_header.slot);

    let update_finalized_slot = update
        .finalized_header
        .as_ref()
        .map(|h| h.slot)
        .unwrap_or(0);

    let update_finalized_period = calc_sync_period(update_finalized_slot);

    let update_has_finalized_next_committee = store.next_sync_committee.is_none()
        && has_sync_update(update)
        && has_finality_update(update)
        && update_finalized_period == update_attested_period;

    let should_apply_update = {
        let has_majority = committee_bits * 3 >= 512 * 2;
        if !has_majority {
            warn!("skipping block with low vote count");
        }

        let update_is_newer = update_finalized_slot > store.finalized_header.slot;
        let good_update = update_is_newer || update_has_finalized_next_committee;

        has_majority && good_update
    };

    if should_apply_update {
        let store_period = calc_sync_period(store.finalized_header.slot);

        if store.next_sync_committee.is_none() {
            store
                .next_sync_committee
                .clone_from(&update.next_sync_committee);
        } else if update_finalized_period == store_period + 1 {
            info!(target: "helios::consensus", "sync committee updated");
            store.current_sync_committee = store.next_sync_committee.clone().unwrap();
            store
                .next_sync_committee
                .clone_from(&update.next_sync_committee);
            store.previous_max_active_participants = store.current_max_active_participants;
            store.current_max_active_participants = 0;
        }

        if update_finalized_slot > store.finalized_header.slot {
            store.finalized_header = update.finalized_header.clone().unwrap();

            if store.finalized_header.slot > store.optimistic_header.slot {
                store.optimistic_header = store.finalized_header.clone();
            }

            if store.finalized_header.slot % 32 == 0 {
                let checkpoint = store.finalized_header.tree_hash_root();
                return Some(checkpoint);
            }
        }
    }

    None
}

// implements checks from validate_light_client_update and process_light_client_update in the
// specification
pub fn verify_generic_update(
    update: &GenericUpdate,
    expected_current_slot: u64,
    store: &LightClientStore,
    genesis_root: B256,
    forks: &Forks,
) -> Result<()> {
    let bits = get_bits(&update.sync_aggregate.sync_committee_bits);
    if bits == 0 {
        return Err(ConsensusError::InsufficientParticipation.into());
    }

    let update_finalized_slot = update.finalized_header.clone().unwrap_or_default().slot;
    let valid_time: bool = expected_current_slot >= update.signature_slot
        && update.signature_slot > update.attested_header.slot
        && update.attested_header.slot >= update_finalized_slot;

    if !valid_time {
        return Err(ConsensusError::InvalidTimestamp.into());
    }

    let store_period = calc_sync_period(store.finalized_header.slot);
    let update_sig_period = calc_sync_period(update.signature_slot);
    let valid_period = if store.next_sync_committee.is_some() {
        update_sig_period == store_period || update_sig_period == store_period + 1
    } else {
        update_sig_period == store_period
    };

    if !valid_period {
        return Err(ConsensusError::InvalidPeriod.into());
    }

    let update_attested_period = calc_sync_period(update.attested_header.slot);
    let update_has_next_committee = store.next_sync_committee.is_none()
        && update.next_sync_committee.is_some()
        && update_attested_period == store_period;

    if update.attested_header.slot <= store.finalized_header.slot && !update_has_next_committee {
        return Err(ConsensusError::NotRelevant.into());
    }

    if update.finalized_header.is_some() && update.finality_branch.is_some() {
        let is_valid = is_finality_proof_valid(
            &update.attested_header,
            &update.finalized_header.clone().unwrap(),
            &update.finality_branch.clone().unwrap(),
        );

        if !is_valid {
            return Err(ConsensusError::InvalidFinalityProof.into());
        }
    }

    if update.next_sync_committee.is_some() && update.next_sync_committee_branch.is_some() {
        let is_valid = is_next_committee_proof_valid(
            &update.attested_header,
            &update.next_sync_committee.clone().unwrap(),
            &update.next_sync_committee_branch.clone().unwrap(),
        );

        if !is_valid {
            return Err(ConsensusError::InvalidNextSyncCommitteeProof.into());
        }
    }

    let sync_committee = if update_sig_period == store_period {
        &store.current_sync_committee
    } else {
        store.next_sync_committee.as_ref().unwrap()
    };

    let pks = get_participating_keys(sync_committee, &update.sync_aggregate.sync_committee_bits)?;

    let fork_version = calculate_fork_version(forks, update.signature_slot);
    let fork_data_root = compute_fork_data_root(fork_version, genesis_root);
    let is_valid_sig = verify_sync_committee_signture(
        &pks,
        &update.attested_header,
        &update.sync_aggregate.sync_committee_signature,
        fork_data_root,
    );

    if !is_valid_sig {
        return Err(ConsensusError::InvalidSignature.into());
    }

    Ok(())
}

pub fn verify_update(
    update: &Update,
    expected_current_slot: u64,
    store: &LightClientStore,
    genesis_root: B256,
    forks: &Forks,
) -> Result<()> {
    let update = GenericUpdate::from(update);

    verify_generic_update(&update, expected_current_slot, store, genesis_root, forks)
}

pub fn verify_finality_update(
    update: &FinalityUpdate,
    expected_current_slot: u64,
    store: &LightClientStore,
    genesis_root: B256,
    forks: &Forks,
) -> Result<()> {
    let update = GenericUpdate::from(update);

    verify_generic_update(&update, expected_current_slot, store, genesis_root, forks)
}

pub fn apply_update(store: &mut LightClientStore, update: &Update) -> Option<B256> {
    let update = GenericUpdate::from(update);
    apply_generic_update(store, &update)
}

pub fn apply_finality_update(
    store: &mut LightClientStore,
    update: &FinalityUpdate,
) -> Option<B256> {
    let update = GenericUpdate::from(update);
    apply_generic_update(store, &update)
}

pub fn apply_optimistic_update(
    store: &mut LightClientStore,
    update: &OptimisticUpdate,
) -> Option<B256> {
    let update = GenericUpdate::from(update);
    apply_generic_update(store, &update)
}

pub fn expected_current_slot(now: SystemTime, genesis_time: u64) -> u64 {
    let now = now.duration_since(UNIX_EPOCH).unwrap();
    let since_genesis = now - std::time::Duration::from_secs(genesis_time);

    since_genesis.as_secs() / 12
}

pub fn verify_sync_committee_signture(
    pks: &[PublicKey],
    attested_header: &Header,
    signature: &Signature,
    fork_data_root: B256,
) -> bool {
    let header_root = attested_header.tree_hash_root();
    let signing_root = compute_committee_sign_root(header_root, fork_data_root);
    signature.verify(signing_root.as_slice(), pks)
}

pub fn compute_committee_sign_root(header: B256, fork_data_root: B256) -> B256 {
    let domain_type = [7, 00, 00, 00];
    let domain = compute_domain(domain_type, fork_data_root);
    compute_signing_root(header, domain)
}

pub fn calculate_fork_version(forks: &Forks, slot: u64) -> FixedVector<u8, typenum::U4> {
    let epoch = slot / 32;

    let version = if epoch >= forks.deneb.epoch {
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
