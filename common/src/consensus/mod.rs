use self::errors::ConsensusError;
use self::types::{
    Bytes32, FinalityUpdate, GenericUpdate, Header, LightClientStore, SignatureBytes,
    SyncCommittee, Update,
};
use self::utils::{
    calc_sync_period, compute_domain, compute_signing_root, is_aggregate_valid, is_proof_valid,
};
use crate::config::types::Forks;
use crate::crypto::bls::PublicKey;
use eyre::Result;
pub mod errors;
pub mod types;
pub mod utils;
use ssz_rs::prelude::*;
use std::cmp;
use tracing::{debug, error, info, warn};
use zduny_wasm_timer::{SystemTime, UNIX_EPOCH};

pub fn get_participating_keys(
    committee: &SyncCommittee,
    bitfield: &Bitvector<512>,
) -> Result<Vec<PublicKey>> {
    let mut pks: Vec<PublicKey> = Vec::new();

    bitfield.iter().enumerate().for_each(|(i, bit)| {
        if bit == true {
            let pk = &committee.pubkeys[i];
            let pk = PublicKey::from_bytes_unchecked(pk).unwrap();
            pks.push(pk);
        }
    });

    Ok(pks)
}

pub fn get_bits(bitfield: &Bitvector<512>) -> u64 {
    let mut count = 0;
    bitfield.iter().for_each(|bit| {
        if bit == true {
            count += 1;
        }
    });

    count
}

pub fn is_finality_proof_valid(
    attested_header: &Header,
    finality_header: &mut Header,
    finality_branch: &[Bytes32],
) -> bool {
    is_proof_valid(attested_header, finality_header, finality_branch, 6, 41)
}

pub fn is_next_committee_proof_valid(
    attested_header: &Header,
    next_committee: &mut SyncCommittee,
    next_committee_branch: &[Bytes32],
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
    current_committee: &mut SyncCommittee,
    current_committee_branch: &[Bytes32],
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
pub fn apply_generic_update(store: &mut LightClientStore, update: &GenericUpdate) {
    let committee_bits = get_bits(&update.sync_aggregate.sync_committee_bits);

    store.current_max_active_participants =
        u64::max(store.current_max_active_participants, committee_bits);

    let should_update_optimistic = committee_bits > safety_threshold(&store)
        && update.attested_header.slot > store.optimistic_header.slot;

    if should_update_optimistic {
        store.optimistic_header = update.attested_header.clone();
        // self.log_optimistic_update(update);
    }

    let update_attested_period = calc_sync_period(update.attested_header.slot.into());

    let update_finalized_slot = update
        .finalized_header
        .as_ref()
        .map(|h| h.slot.as_u64())
        .unwrap_or(0);

    let update_finalized_period = calc_sync_period(update_finalized_slot);

    let update_has_finalized_next_committee = store.next_sync_committee.is_none()
        && has_sync_update(update)
        && has_finality_update(update)
        && update_finalized_period == update_attested_period;

    let should_apply_update = {
        let has_majority = committee_bits * 3 >= 512 * 2;
        if !has_majority {
            tracing::warn!("skipping block with low vote count");
        }

        let update_is_newer = update_finalized_slot > store.finalized_header.slot.as_u64();
        let good_update = update_is_newer || update_has_finalized_next_committee;

        has_majority && good_update
    };

    if should_apply_update {
        let store_period = calc_sync_period(store.finalized_header.slot.into());

        if store.next_sync_committee.is_none() {
            store.next_sync_committee = update.next_sync_committee.clone();
        } else if update_finalized_period == store_period + 1 {
            info!(target: "helios::consensus", "sync committee updated");
            store.current_sync_committee = store.next_sync_committee.clone().unwrap();
            store.next_sync_committee = update.next_sync_committee.clone();
            store.previous_max_active_participants = store.current_max_active_participants;
            store.current_max_active_participants = 0;
        }

        if update_finalized_slot > store.finalized_header.slot.as_u64() {
            store.finalized_header = update.finalized_header.clone().unwrap();
            // log_finality_update(update);

            if store.finalized_header.slot.as_u64() % 32 == 0 {
                let checkpoint_res = store.finalized_header.hash_tree_root();
                if let Ok(checkpoint) = checkpoint_res {
                    // TOOD: Figure out log_update and self.last_checkpoint for zkVM
                    println!("DEBUG: Should have updated last_checkpoint ");
                    // self.last_checkpoint = Some(checkpoint.as_ref().to_vec());
                }
            }

            if store.finalized_header.slot > store.optimistic_header.slot {
                store.optimistic_header = store.finalized_header.clone();
            }
        }
    }
}

// implements checks from validate_light_client_update and process_light_client_update in the
// specification
pub fn verify_generic_update(
    update: &GenericUpdate,
    now: SystemTime,
    genesis_time: u64,
    store: &LightClientStore,
    genesis_root: Vec<u8>,
    forks: &Forks,
) -> Result<()> {
    let bits = get_bits(&update.sync_aggregate.sync_committee_bits);
    if bits == 0 {
        return Err(ConsensusError::InsufficientParticipation.into());
    }

    let update_finalized_slot = update.finalized_header.clone().unwrap_or_default().slot;
    let valid_time = expected_current_slot(now, genesis_time) >= update.signature_slot
        && update.signature_slot > update.attested_header.slot.as_u64()
        && update.attested_header.slot >= update_finalized_slot;

    if !valid_time {
        return Err(ConsensusError::InvalidTimestamp.into());
    }

    let store_period = calc_sync_period(store.finalized_header.slot.into());
    let update_sig_period = calc_sync_period(update.signature_slot);
    let valid_period = if store.next_sync_committee.is_some() {
        update_sig_period == store_period || update_sig_period == store_period + 1
    } else {
        update_sig_period == store_period
    };

    if !valid_period {
        return Err(ConsensusError::InvalidPeriod.into());
    }

    let update_attested_period = calc_sync_period(update.attested_header.slot.into());
    let update_has_next_committee = store.next_sync_committee.is_none()
        && update.next_sync_committee.is_some()
        && update_attested_period == store_period;

    if update.attested_header.slot <= store.finalized_header.slot && !update_has_next_committee {
        return Err(ConsensusError::NotRelevant.into());
    }

    if update.finalized_header.is_some() && update.finality_branch.is_some() {
        let is_valid = is_finality_proof_valid(
            &update.attested_header,
            &mut update.finalized_header.clone().unwrap(),
            &update.finality_branch.clone().unwrap(),
        );

        if !is_valid {
            return Err(ConsensusError::InvalidFinalityProof.into());
        }
    }

    if update.next_sync_committee.is_some() && update.next_sync_committee_branch.is_some() {
        let is_valid = is_next_committee_proof_valid(
            &update.attested_header,
            &mut update.next_sync_committee.clone().unwrap(),
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

    let is_valid_sig = verify_sync_committee_signture(
        &pks,
        &update.attested_header,
        &update.sync_aggregate.sync_committee_signature,
        update.signature_slot,
        genesis_root,
        forks,
    );

    if !is_valid_sig {
        return Err(ConsensusError::InvalidSignature.into());
    }

    Ok(())
}

pub fn verify_update(
    update: &Update,
    now: SystemTime,
    genesis_time: u64,
    store: &LightClientStore,
    genesis_root: Vec<u8>,
    forks: &Forks,
) -> Result<()> {
    let update = GenericUpdate::from(update);

    verify_generic_update(&update, now, genesis_time, store, genesis_root, forks)
}

pub fn verify_finality_update(
    update: &FinalityUpdate,
    now: SystemTime,
    genesis_time: u64,
    store: &LightClientStore,
    genesis_root: Vec<u8>,
    forks: &Forks,
) -> Result<()> {
    let update = GenericUpdate::from(update);

    verify_generic_update(&update, now, genesis_time, store, genesis_root, forks)
}

pub fn apply_update(store: &mut LightClientStore, update: &Update) {
    let update = GenericUpdate::from(update);
    apply_generic_update(store, &update);
}

pub fn apply_finality_update(store: &mut LightClientStore, update: &FinalityUpdate) {
    let update = GenericUpdate::from(update);
    apply_generic_update(store, &update);
}

pub fn expected_current_slot(now: SystemTime, genesis_time: u64) -> u64 {
    let now = now.duration_since(UNIX_EPOCH).unwrap();
    let since_genesis = now - std::time::Duration::from_secs(genesis_time);

    since_genesis.as_secs() / 12
}

pub fn verify_sync_committee_signture(
    pks: &[PublicKey],
    attested_header: &Header,
    signature: &SignatureBytes,
    signature_slot: u64,
    genesis_root: Vec<u8>,
    forks: &Forks,
) -> bool {
    let res: Result<bool> = (move || {
        let pks: Vec<&PublicKey> = pks.iter().collect();
        let header_root = Bytes32::try_from(attested_header.clone().hash_tree_root()?.as_ref())?;
        let signing_root =
            compute_committee_sign_root(header_root, signature_slot, genesis_root, forks)?;

        Ok(is_aggregate_valid(signature, signing_root.as_ref(), &pks))
    })();

    if let Ok(is_valid) = res {
        is_valid
    } else {
        false
    }
}

pub fn compute_committee_sign_root(
    header: Bytes32,
    slot: u64,
    genesis_root: Vec<u8>,
    forks: &Forks,
) -> Result<Node> {
    let genesis_root = genesis_root.to_vec().try_into().unwrap();

    let domain_type = &hex::decode("07000000")?[..];
    let fork_version =
        Vector::try_from(calculate_fork_version(forks, slot)).map_err(|(_, err)| err)?;
    let domain = compute_domain(domain_type, fork_version, genesis_root)?;
    compute_signing_root(header, domain)
}

pub fn calculate_fork_version(forks: &Forks, slot: u64) -> Vec<u8> {
    let epoch = slot / 32;

    if epoch >= forks.deneb.epoch {
        forks.deneb.fork_version.clone()
    } else if epoch >= forks.capella.epoch {
        forks.capella.fork_version.clone()
    } else if epoch >= forks.bellatrix.epoch {
        forks.bellatrix.fork_version.clone()
    } else if epoch >= forks.altair.epoch {
        forks.altair.fork_version.clone()
    } else {
        forks.genesis.fork_version.clone()
    }
}
