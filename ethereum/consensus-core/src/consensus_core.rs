use std::cmp;

use alloy::primitives::B256;
use eyre::Result;
use ssz_types::BitVector;
use tracing::{info, warn};
use tree_hash::TreeHash;
use zduny_wasm_timer::{SystemTime, UNIX_EPOCH};

use crate::errors::ConsensusError;
use crate::proof::{
    is_current_committee_proof_valid, is_execution_payload_proof_valid, is_finality_proof_valid,
    is_next_committee_proof_valid,
};
use crate::types::bls::{PublicKey, Signature};
use crate::types::{
    BeaconBlockHeader, Bootstrap, ExecutionPayloadHeader, FinalityUpdate, Forks, GenericUpdate,
    LightClientHeader, LightClientStore, OptimisticUpdate, Update,
};
use crate::utils::{
    calculate_fork_version, compute_committee_sign_root, compute_fork_data_root,
    get_participating_keys,
};

pub fn verify_bootstrap(bootstrap: &Bootstrap, checkpoint: B256, forks: &Forks) -> Result<()> {
    if !is_valid_header(&bootstrap.header, forks) {
        return Err(ConsensusError::InvalidExecutionPayloadProof.into());
    }

    let committee_valid = is_current_committee_proof_valid(
        &bootstrap.header.beacon,
        &bootstrap.current_sync_committee,
        &bootstrap.current_sync_committee_branch,
    );

    let header_hash = bootstrap.header.beacon.tree_hash_root();
    let header_valid = header_hash == checkpoint;

    if !header_valid {
        return Err(ConsensusError::InvalidHeaderHash(checkpoint, header_hash).into());
    }

    if !committee_valid {
        return Err(ConsensusError::InvalidCurrentSyncCommitteeProof.into());
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

pub fn verify_optimistic_update(
    update: &OptimisticUpdate,
    expected_current_slot: u64,
    store: &LightClientStore,
    genesis_root: B256,
    forks: &Forks,
) -> Result<()> {
    let update = GenericUpdate::from(update);
    verify_generic_update(&update, expected_current_slot, store, genesis_root, forks)
}

pub fn apply_bootstrap(store: &mut LightClientStore, bootstrap: &Bootstrap) {
    *store = LightClientStore {
        finalized_header: bootstrap.header.clone(),
        current_sync_committee: bootstrap.current_sync_committee.clone(),
        next_sync_committee: None,
        optimistic_header: bootstrap.header.clone(),
        previous_max_active_participants: 0,
        current_max_active_participants: 0,
    };
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

// implements state changes from apply_light_client_update and process_light_client_update in
// the specification
/// Returns the new checkpoint if one is created, otherwise None
fn apply_generic_update(store: &mut LightClientStore, update: &GenericUpdate) -> Option<B256> {
    let committee_bits = get_bits(&update.sync_aggregate.sync_committee_bits);

    store.current_max_active_participants =
        u64::max(store.current_max_active_participants, committee_bits);

    let should_update_optimistic = committee_bits > safety_threshold(store)
        && update.attested_header.beacon.slot > store.optimistic_header.beacon.slot;

    if should_update_optimistic {
        store.optimistic_header = update.attested_header.clone();
    }

    let update_attested_period = calc_sync_period(update.attested_header.beacon.slot);

    let update_finalized_slot = update
        .finalized_header
        .as_ref()
        .map(|h| h.beacon.slot)
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

        let update_is_newer = update_finalized_slot > store.finalized_header.beacon.slot;
        let good_update = update_is_newer || update_has_finalized_next_committee;

        has_majority && good_update
    };

    if should_apply_update {
        let store_period = calc_sync_period(store.finalized_header.beacon.slot);

        if store.next_sync_committee.is_none() {
            if update_finalized_period != store_period {
                return None;
            }
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

        if update_finalized_slot > store.finalized_header.beacon.slot {
            store.finalized_header = update.finalized_header.clone().unwrap();

            if store.finalized_header.beacon.slot > store.optimistic_header.beacon.slot {
                store.optimistic_header = store.finalized_header.clone();
            }

            if store.finalized_header.beacon.slot % 32 == 0 {
                let checkpoint = store.finalized_header.beacon.tree_hash_root();
                return Some(checkpoint);
            }
        }
    }

    None
}

// implements checks from validate_light_client_update and process_light_client_update in the
// specification
fn verify_generic_update(
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

    if !is_valid_header(&update.attested_header, forks) {
        return Err(ConsensusError::InvalidExecutionPayloadProof.into());
    }

    let update_finalized_slot = update
        .finalized_header
        .clone()
        .unwrap_or_default()
        .beacon
        .slot;

    let valid_time: bool = expected_current_slot >= update.signature_slot
        && update.signature_slot > update.attested_header.beacon.slot
        && update.attested_header.beacon.slot >= update_finalized_slot;

    if !valid_time {
        return Err(ConsensusError::InvalidTimestamp.into());
    }

    let store_period = calc_sync_period(store.finalized_header.beacon.slot);
    let update_sig_period = calc_sync_period(update.signature_slot);
    let valid_period = if store.next_sync_committee.is_some() {
        update_sig_period == store_period || update_sig_period == store_period + 1
    } else {
        update_sig_period == store_period
    };

    if !valid_period {
        return Err(ConsensusError::InvalidPeriod.into());
    }

    let update_attested_period = calc_sync_period(update.attested_header.beacon.slot);
    let update_has_next_committee = store.next_sync_committee.is_none()
        && update.next_sync_committee.is_some()
        && update_attested_period == store_period;

    if update.attested_header.beacon.slot <= store.finalized_header.beacon.slot
        && !update_has_next_committee
    {
        return Err(ConsensusError::NotRelevant.into());
    }

    if let Some(finalized_header) = &update.finalized_header {
        if let Some(finality_branch) = &update.finality_branch {
            if !is_valid_header(finalized_header, forks) {
                return Err(ConsensusError::InvalidExecutionPayloadProof.into());
            }

            let is_valid = is_finality_proof_valid(
                &update.attested_header.beacon,
                &finalized_header.beacon,
                finality_branch,
            );

            if !is_valid {
                return Err(ConsensusError::InvalidFinalityProof.into());
            }
        } else {
            return Err(ConsensusError::InvalidFinalityProof.into());
        }
    }

    if let Some(next_sync_committee) = &update.next_sync_committee {
        if let Some(next_sync_committee_branch) = &update.next_sync_committee_branch {
            let is_valid = is_next_committee_proof_valid(
                &update.attested_header.beacon,
                next_sync_committee,
                next_sync_committee_branch,
            );

            if !is_valid {
                return Err(ConsensusError::InvalidNextSyncCommitteeProof.into());
            }
        } else {
            return Err(ConsensusError::InvalidNextSyncCommitteeProof.into());
        }
    }

    let sync_committee = if update_sig_period == store_period {
        &store.current_sync_committee
    } else {
        store.next_sync_committee.as_ref().unwrap()
    };

    let pks = get_participating_keys(sync_committee, &update.sync_aggregate.sync_committee_bits)?;

    let fork_version = calculate_fork_version(forks, update.signature_slot.saturating_sub(1));
    let fork_data_root = compute_fork_data_root(fork_version, genesis_root);
    let is_valid_sig = verify_sync_committee_signture(
        &pks,
        &update.attested_header.beacon,
        &update.sync_aggregate.sync_committee_signature,
        fork_data_root,
    );

    if !is_valid_sig {
        return Err(ConsensusError::InvalidSignature.into());
    }

    Ok(())
}

pub fn expected_current_slot(now: SystemTime, genesis_time: u64) -> u64 {
    let now = now.duration_since(UNIX_EPOCH).unwrap();
    let since_genesis = now - std::time::Duration::from_secs(genesis_time);

    since_genesis.as_secs() / 12
}

pub fn calc_sync_period(slot: u64) -> u64 {
    // 32 slots per epoch
    let epoch = slot / 32;
    // 256 epochs per sync committee
    epoch / 256
}

pub fn get_bits(bitfield: &BitVector<typenum::U512>) -> u64 {
    bitfield.iter().filter(|v| *v).count() as u64
}

fn has_sync_update(update: &GenericUpdate) -> bool {
    update.next_sync_committee.is_some() && update.next_sync_committee_branch.is_some()
}

fn has_finality_update(update: &GenericUpdate) -> bool {
    update.finalized_header.is_some() && update.finality_branch.is_some()
}

fn verify_sync_committee_signture(
    pks: &[PublicKey],
    attested_header: &BeaconBlockHeader,
    signature: &Signature,
    fork_data_root: B256,
) -> bool {
    let header_root = attested_header.tree_hash_root();
    let signing_root = compute_committee_sign_root(header_root, fork_data_root);
    signature.verify(signing_root.as_slice(), pks)
}

fn safety_threshold(store: &LightClientStore) -> u64 {
    cmp::max(
        store.current_max_active_participants,
        store.previous_max_active_participants,
    ) / 2
}

fn is_valid_header(header: &LightClientHeader, forks: &Forks) -> bool {
    let epoch = header.beacon.slot / 32;

    if epoch < forks.capella.epoch {
        header.execution.is_none() && header.execution_branch.is_none()
    } else if header.execution.is_some() && header.execution_branch.is_some() {
        let execution = header.execution.as_ref().unwrap();
        let execution_branch = header.execution_branch.as_ref().unwrap();

        let valid_execution_type = match execution {
            ExecutionPayloadHeader::Deneb(_) => epoch >= forks.deneb.epoch,
            ExecutionPayloadHeader::Capella(_) => {
                epoch >= forks.capella.epoch && epoch < forks.deneb.epoch
            }
            ExecutionPayloadHeader::Bellatrix(_) => {
                epoch >= forks.bellatrix.epoch && epoch < forks.altair.epoch
            }
        };

        let proof_valid =
            is_execution_payload_proof_valid(&header.beacon, execution, execution_branch);

        proof_valid && valid_execution_type
    } else {
        false
    }
}
