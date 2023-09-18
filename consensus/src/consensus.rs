use std::cmp;
use std::marker::PhantomData;
use std::process;
use std::sync::Arc;

use chrono::Duration;
use eyre::eyre;
use eyre::Result;
use futures::future::join_all;
use milagro_bls::PublicKey;
use ssz_rs::prelude::*;
use tokio::sync::mpsc::Sender;
use tracing::{debug, error, info, warn};
use wasm_timer::{SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc::channel;
use tokio::sync::mpsc::Receiver;
use tokio::sync::watch;
use tokio::time::sleep;

use common::types::Block;
use config::CheckpointFallback;
use config::Config;
use config::Network;

use crate::constants::MAX_REQUEST_LIGHT_CLIENT_UPDATES;
use crate::database::Database;
use crate::errors::ConsensusError;

use super::rpc::ConsensusRpc;
use super::types::*;
use super::utils::*;

pub struct ConsensusClient<R: ConsensusRpc, DB: Database> {
    pub block_recv: Option<Receiver<Block>>,
    pub finalized_block_recv: Option<watch::Receiver<Option<Block>>>,
    pub checkpoint_recv: watch::Receiver<Option<Vec<u8>>>,
    genesis_time: u64,
    db: DB,
    phantom: PhantomData<R>,
}

#[derive(Debug)]
pub struct Inner<R: ConsensusRpc> {
    rpc: R,
    store: LightClientStore,
    last_checkpoint: Option<Vec<u8>>,
    block_send: Sender<Block>,
    finalized_block_send: watch::Sender<Option<Block>>,
    checkpoint_send: watch::Sender<Option<Vec<u8>>>,
    pub config: Arc<Config>,
}

#[derive(Debug, Default)]
struct LightClientStore {
    finalized_header: Header,
    current_sync_committee: SyncCommittee,
    next_sync_committee: Option<SyncCommittee>,
    optimistic_header: Header,
    previous_max_active_participants: u64,
    current_max_active_participants: u64,
}

impl<R: ConsensusRpc, DB: Database> ConsensusClient<R, DB> {
    pub fn new(rpc: &str, config: Arc<Config>) -> Result<ConsensusClient<R, DB>> {
        let (block_send, block_recv) = channel(256);
        let (finalized_block_send, finalized_block_recv) = watch::channel(None);
        let (checkpoint_send, checkpoint_recv) = watch::channel(None);

        let rpc = rpc.to_string();
        let genesis_time = config.chain.genesis_time;
        let db = DB::new(&config)?;
        let initial_checkpoint = config.checkpoint.clone().unwrap_or_else(|| {
            db.load_checkpoint()
                .unwrap_or(config.default_checkpoint.clone())
        });

        tokio::spawn(async move {
            let mut inner = Inner::<R>::new(
                &rpc,
                block_send,
                finalized_block_send,
                checkpoint_send,
                config.clone(),
            );

            let res = inner.sync(&initial_checkpoint).await;
            if let Err(err) = res {
                if config.load_external_fallback {
                    let res = sync_all_fallbacks(&mut inner, config.chain.chain_id).await;
                    if let Err(err) = res {
                        error!(target: "helios::consensus", err = %err, "sync failed");
                        process::exit(1);
                    }
                } else if let Some(fallback) = &config.fallback {
                    let res = sync_fallback(&mut inner, fallback).await;
                    if let Err(err) = res {
                        error!(target: "helios::consensus", err = %err, "sync failed");
                        process::exit(1);
                    }
                } else {
                    error!(target: "helios::consensus", err = %err, "sync failed");
                    process::exit(1);
                }
            }

            _ = inner.send_blocks().await;

            loop {
                sleep(inner.duration_until_next_update().to_std().unwrap()).await;

                let res = inner.advance().await;
                if let Err(err) = res {
                    warn!(target: "helios::consensus", "advance error: {}", err);
                    continue;
                }

                let res = inner.send_blocks().await;
                if let Err(err) = res {
                    warn!(target: "helios::consensus", "send error: {}", err);
                    continue;
                }
            }
        });

        Ok(ConsensusClient {
            block_recv: Some(block_recv),
            finalized_block_recv: Some(finalized_block_recv),
            checkpoint_recv,
            genesis_time,
            db,
            phantom: PhantomData,
        })
    }

    pub fn shutdown(&self) -> Result<()> {
        let checkpoint = self.checkpoint_recv.borrow();
        if let Some(checkpoint) = checkpoint.as_ref() {
            self.db.save_checkpoint(checkpoint)?;
        }

        Ok(())
    }

    pub fn expected_current_slot(&self) -> u64 {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let since_genesis = now - std::time::Duration::from_secs(self.genesis_time);

        since_genesis.as_secs() / 12
    }
}

async fn sync_fallback<R: ConsensusRpc>(inner: &mut Inner<R>, fallback: &str) -> Result<()> {
    let checkpoint = CheckpointFallback::fetch_checkpoint_from_api(fallback).await?;
    inner.sync(checkpoint.as_bytes()).await
}

async fn sync_all_fallbacks<R: ConsensusRpc>(inner: &mut Inner<R>, chain_id: u64) -> Result<()> {
    let network = Network::from_chain_id(chain_id)?;
    let checkpoint = CheckpointFallback::new()
        .build()
        .await?
        .fetch_latest_checkpoint(&network)
        .await?;

    inner.sync(checkpoint.as_bytes()).await
}

impl<R: ConsensusRpc> Inner<R> {
    pub fn new(
        rpc: &str,
        block_send: Sender<Block>,
        finalized_block_send: watch::Sender<Option<Block>>,
        checkpoint_send: watch::Sender<Option<Vec<u8>>>,
        config: Arc<Config>,
    ) -> Inner<R> {
        let rpc = R::new(rpc);

        Inner {
            rpc,
            store: LightClientStore::default(),
            last_checkpoint: None,
            block_send,
            finalized_block_send,
            checkpoint_send,
            config,
        }
    }

    pub async fn check_rpc(&self) -> Result<()> {
        let chain_id = self.rpc.chain_id().await?;

        if chain_id != self.config.chain.chain_id {
            Err(ConsensusError::IncorrectRpcNetwork.into())
        } else {
            Ok(())
        }
    }

    pub async fn get_execution_payload(&self, slot: &Option<u64>) -> Result<ExecutionPayload> {
        let slot = slot.unwrap_or(self.store.optimistic_header.slot.into());
        let mut block = self.rpc.get_block(slot).await?;
        let block_hash = block.hash_tree_root()?;

        let latest_slot = self.store.optimistic_header.slot;
        let finalized_slot = self.store.finalized_header.slot;

        let verified_block_hash = if slot == latest_slot.as_u64() {
            self.store.optimistic_header.clone().hash_tree_root()?
        } else if slot == finalized_slot.as_u64() {
            self.store.finalized_header.clone().hash_tree_root()?
        } else {
            return Err(ConsensusError::PayloadNotFound(slot).into());
        };

        if verified_block_hash != block_hash {
            Err(ConsensusError::InvalidHeaderHash(
                block_hash.to_string(),
                verified_block_hash.to_string(),
            )
            .into())
        } else {
            Ok(block.body.execution_payload().clone())
        }
    }

    pub async fn get_payloads(
        &self,
        start_slot: u64,
        end_slot: u64,
    ) -> Result<Vec<ExecutionPayload>> {
        let payloads_fut = (start_slot..end_slot)
            .rev()
            .map(|slot| self.rpc.get_block(slot));

        let mut prev_parent_hash: Bytes32 = self
            .rpc
            .get_block(end_slot)
            .await?
            .body
            .execution_payload()
            .parent_hash()
            .clone();

        let mut payloads: Vec<ExecutionPayload> = Vec::new();
        for result in join_all(payloads_fut).await {
            if result.is_err() {
                continue;
            }
            let payload = result.unwrap().body.execution_payload().clone();
            if payload.block_hash() != &prev_parent_hash {
                warn!(
                    target: "helios::consensus",
                    error = %ConsensusError::InvalidHeaderHash(
                        format!("{prev_parent_hash:02X?}"),
                        format!("{:02X?}", payload.parent_hash()),
                    ),
                    "error while backfilling blocks"
                );
                break;
            }
            prev_parent_hash = payload.parent_hash().clone();
            payloads.push(payload);
        }
        Ok(payloads)
    }

    pub async fn sync(&mut self, checkpoint: &[u8]) -> Result<()> {
        self.store = LightClientStore::default();
        self.last_checkpoint = None;

        self.bootstrap(checkpoint).await?;

        let current_period = calc_sync_period(self.store.finalized_header.slot.into());
        let updates = self
            .rpc
            .get_updates(current_period, MAX_REQUEST_LIGHT_CLIENT_UPDATES)
            .await?;

        for update in updates {
            self.verify_update(&update)?;
            self.apply_update(&update);
        }

        let finality_update = self.rpc.get_finality_update().await?;
        self.verify_finality_update(&finality_update)?;
        self.apply_finality_update(&finality_update);

        let optimistic_update = self.rpc.get_optimistic_update().await?;
        self.verify_optimistic_update(&optimistic_update)?;
        self.apply_optimistic_update(&optimistic_update);

        info!(
            target: "helios::consensus",
            "consensus client in sync with checkpoint: 0x{}",
            hex::encode(checkpoint)
        );

        Ok(())
    }

    pub async fn advance(&mut self) -> Result<()> {
        let finality_update = self.rpc.get_finality_update().await?;
        self.verify_finality_update(&finality_update)?;
        self.apply_finality_update(&finality_update);

        let optimistic_update = self.rpc.get_optimistic_update().await?;
        self.verify_optimistic_update(&optimistic_update)?;
        self.apply_optimistic_update(&optimistic_update);

        if self.store.next_sync_committee.is_none() {
            debug!(target: "helios::consensus", "checking for sync committee update");
            let current_period = calc_sync_period(self.store.finalized_header.slot.into());
            let mut updates = self.rpc.get_updates(current_period, 1).await?;

            if updates.len() == 1 {
                let update = updates.get_mut(0).unwrap();
                let res = self.verify_update(update);

                if res.is_ok() {
                    info!(target: "helios::consensus", "updating sync committee");
                    self.apply_update(update);
                }
            }
        }

        Ok(())
    }

    pub async fn send_blocks(&self) -> Result<()> {
        let slot = self.store.optimistic_header.slot.as_u64();
        let payload = self.get_execution_payload(&Some(slot)).await?;
        let finalized_slot = self.store.finalized_header.slot.as_u64();
        let finalized_payload = self.get_execution_payload(&Some(finalized_slot)).await?;

        self.block_send.send(payload.into()).await?;
        self.finalized_block_send
            .send(Some(finalized_payload.into()))?;
        self.checkpoint_send.send(self.last_checkpoint.clone())?;

        Ok(())
    }

    /// Gets the duration until the next update
    /// Updates are scheduled for 4 seconds into each slot
    pub fn duration_until_next_update(&self) -> Duration {
        let current_slot = self.expected_current_slot();
        let next_slot = current_slot + 1;
        let next_slot_timestamp = self.slot_timestamp(next_slot);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let time_to_next_slot = next_slot_timestamp - now;
        let next_update = time_to_next_slot + 4;

        Duration::seconds(next_update as i64)
    }

    async fn bootstrap(&mut self, checkpoint: &[u8]) -> Result<()> {
        let mut bootstrap = self
            .rpc
            .get_bootstrap(checkpoint)
            .await
            .map_err(|_| eyre!("could not fetch bootstrap"))?;

        let is_valid = self.is_valid_checkpoint(bootstrap.header.slot.into());

        if !is_valid {
            if self.config.strict_checkpoint_age {
                return Err(ConsensusError::CheckpointTooOld.into());
            } else {
                warn!(target: "helios::consensus", "checkpoint too old, consider using a more recent block");
            }
        }

        let committee_valid = is_current_committee_proof_valid(
            &bootstrap.header,
            &mut bootstrap.current_sync_committee,
            &bootstrap.current_sync_committee_branch,
        );

        let header_hash = bootstrap.header.hash_tree_root()?.to_string();
        let expected_hash = format!("0x{}", hex::encode(checkpoint));
        let header_valid = header_hash == expected_hash;

        if !header_valid {
            return Err(ConsensusError::InvalidHeaderHash(expected_hash, header_hash).into());
        }

        if !committee_valid {
            return Err(ConsensusError::InvalidCurrentSyncCommitteeProof.into());
        }

        self.store = LightClientStore {
            finalized_header: bootstrap.header.clone(),
            current_sync_committee: bootstrap.current_sync_committee,
            next_sync_committee: None,
            optimistic_header: bootstrap.header.clone(),
            previous_max_active_participants: 0,
            current_max_active_participants: 0,
        };

        Ok(())
    }

    // implements checks from validate_light_client_update and process_light_client_update in the
    // specification
    fn verify_generic_update(&self, update: &GenericUpdate) -> Result<()> {
        let bits = get_bits(&update.sync_aggregate.sync_committee_bits);
        if bits == 0 {
            return Err(ConsensusError::InsufficientParticipation.into());
        }

        let update_finalized_slot = update.finalized_header.clone().unwrap_or_default().slot;
        let valid_time = self.expected_current_slot() >= update.signature_slot
            && update.signature_slot > update.attested_header.slot.as_u64()
            && update.attested_header.slot >= update_finalized_slot;

        if !valid_time {
            return Err(ConsensusError::InvalidTimestamp.into());
        }

        let store_period = calc_sync_period(self.store.finalized_header.slot.into());
        let update_sig_period = calc_sync_period(update.signature_slot);
        let valid_period = if self.store.next_sync_committee.is_some() {
            update_sig_period == store_period || update_sig_period == store_period + 1
        } else {
            update_sig_period == store_period
        };

        if !valid_period {
            return Err(ConsensusError::InvalidPeriod.into());
        }

        let update_attested_period = calc_sync_period(update.attested_header.slot.into());
        let update_has_next_committee = self.store.next_sync_committee.is_none()
            && update.next_sync_committee.is_some()
            && update_attested_period == store_period;

        if update.attested_header.slot <= self.store.finalized_header.slot
            && !update_has_next_committee
        {
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
            &self.store.current_sync_committee
        } else {
            self.store.next_sync_committee.as_ref().unwrap()
        };

        let pks =
            get_participating_keys(sync_committee, &update.sync_aggregate.sync_committee_bits)?;

        let is_valid_sig = self.verify_sync_committee_signture(
            &pks,
            &update.attested_header,
            &update.sync_aggregate.sync_committee_signature,
            update.signature_slot,
        );

        if !is_valid_sig {
            return Err(ConsensusError::InvalidSignature.into());
        }

        Ok(())
    }

    fn verify_update(&self, update: &Update) -> Result<()> {
        let update = GenericUpdate::from(update);
        self.verify_generic_update(&update)
    }

    fn verify_finality_update(&self, update: &FinalityUpdate) -> Result<()> {
        let update = GenericUpdate::from(update);
        self.verify_generic_update(&update)
    }

    fn verify_optimistic_update(&self, update: &OptimisticUpdate) -> Result<()> {
        let update = GenericUpdate::from(update);
        self.verify_generic_update(&update)
    }

    // implements state changes from apply_light_client_update and process_light_client_update in
    // the specification
    fn apply_generic_update(&mut self, update: &GenericUpdate) {
        let committee_bits = get_bits(&update.sync_aggregate.sync_committee_bits);

        self.store.current_max_active_participants =
            u64::max(self.store.current_max_active_participants, committee_bits);

        let should_update_optimistic = committee_bits > self.safety_threshold()
            && update.attested_header.slot > self.store.optimistic_header.slot;

        if should_update_optimistic {
            self.store.optimistic_header = update.attested_header.clone();
            self.log_optimistic_update(update);
        }

        let update_attested_period = calc_sync_period(update.attested_header.slot.into());

        let update_finalized_slot = update
            .finalized_header
            .as_ref()
            .map(|h| h.slot.as_u64())
            .unwrap_or(0);

        let update_finalized_period = calc_sync_period(update_finalized_slot);

        let update_has_finalized_next_committee = self.store.next_sync_committee.is_none()
            && self.has_sync_update(update)
            && self.has_finality_update(update)
            && update_finalized_period == update_attested_period;

        let should_apply_update = {
            let has_majority = committee_bits * 3 >= 512 * 2;
            let update_is_newer = update_finalized_slot > self.store.finalized_header.slot.as_u64();
            let good_update = update_is_newer || update_has_finalized_next_committee;

            has_majority && good_update
        };

        if should_apply_update {
            let store_period = calc_sync_period(self.store.finalized_header.slot.into());

            if self.store.next_sync_committee.is_none() {
                self.store.next_sync_committee = update.next_sync_committee.clone();
            } else if update_finalized_period == store_period + 1 {
                info!(target: "helios::consensus", "sync committee updated");
                self.store.current_sync_committee = self.store.next_sync_committee.clone().unwrap();
                self.store.next_sync_committee = update.next_sync_committee.clone();
                self.store.previous_max_active_participants =
                    self.store.current_max_active_participants;
                self.store.current_max_active_participants = 0;
            }

            if update_finalized_slot > self.store.finalized_header.slot.as_u64() {
                self.store.finalized_header = update.finalized_header.clone().unwrap();
                self.log_finality_update(update);

                if self.store.finalized_header.slot.as_u64() % 32 == 0 {
                    let checkpoint_res = self.store.finalized_header.hash_tree_root();
                    if let Ok(checkpoint) = checkpoint_res {
                        self.last_checkpoint = Some(checkpoint.as_ref().to_vec());
                    }
                }

                if self.store.finalized_header.slot > self.store.optimistic_header.slot {
                    self.store.optimistic_header = self.store.finalized_header.clone();
                }
            }
        }
    }

    fn apply_update(&mut self, update: &Update) {
        let update = GenericUpdate::from(update);
        self.apply_generic_update(&update);
    }

    fn apply_finality_update(&mut self, update: &FinalityUpdate) {
        let update = GenericUpdate::from(update);
        self.apply_generic_update(&update);
    }

    fn log_finality_update(&self, update: &GenericUpdate) {
        let participation =
            get_bits(&update.sync_aggregate.sync_committee_bits) as f32 / 512_f32 * 100f32;
        let decimals = if participation == 100.0 { 1 } else { 2 };
        let age = self.age(self.store.finalized_header.slot.as_u64());

        info!(
            target: "helios::consensus",
            "finalized slot             slot={}  confidence={:.decimals$}%  age={:02}:{:02}:{:02}:{:02}",
            self.store.finalized_header.slot.as_u64(),
            participation,
            age.num_days(),
            age.num_hours() % 24,
            age.num_minutes() % 60,
            age.num_seconds() % 60,
        );
    }

    fn apply_optimistic_update(&mut self, update: &OptimisticUpdate) {
        let update = GenericUpdate::from(update);
        self.apply_generic_update(&update);
    }

    fn log_optimistic_update(&self, update: &GenericUpdate) {
        let participation =
            get_bits(&update.sync_aggregate.sync_committee_bits) as f32 / 512_f32 * 100f32;
        let decimals = if participation == 100.0 { 1 } else { 2 };
        let age = self.age(self.store.optimistic_header.slot.as_u64());

        info!(
            target: "helios::consensus",
            "updated head               slot={}  confidence={:.decimals$}%  age={:02}:{:02}:{:02}:{:02}",
            self.store.optimistic_header.slot.as_u64(),
            participation,
            age.num_days(),
            age.num_hours() % 24,
            age.num_minutes() % 60,
            age.num_seconds() % 60,
        );
    }

    fn has_finality_update(&self, update: &GenericUpdate) -> bool {
        update.finalized_header.is_some() && update.finality_branch.is_some()
    }

    fn has_sync_update(&self, update: &GenericUpdate) -> bool {
        update.next_sync_committee.is_some() && update.next_sync_committee_branch.is_some()
    }

    fn safety_threshold(&self) -> u64 {
        cmp::max(
            self.store.current_max_active_participants,
            self.store.previous_max_active_participants,
        ) / 2
    }

    fn verify_sync_committee_signture(
        &self,
        pks: &[PublicKey],
        attested_header: &Header,
        signature: &SignatureBytes,
        signature_slot: u64,
    ) -> bool {
        let res: Result<bool> = (move || {
            let pks: Vec<&PublicKey> = pks.iter().collect();
            let header_root =
                Bytes32::try_from(attested_header.clone().hash_tree_root()?.as_ref())?;
            let signing_root = self.compute_committee_sign_root(header_root, signature_slot)?;

            Ok(is_aggregate_valid(signature, signing_root.as_ref(), &pks))
        })();

        if let Ok(is_valid) = res {
            is_valid
        } else {
            false
        }
    }

    fn compute_committee_sign_root(&self, header: Bytes32, slot: u64) -> Result<Node> {
        let genesis_root = self.config.chain.genesis_root.to_vec().try_into().unwrap();

        let domain_type = &hex::decode("07000000")?[..];
        let fork_version =
            Vector::try_from(self.config.fork_version(slot)).map_err(|(_, err)| err)?;
        let domain = compute_domain(domain_type, fork_version, genesis_root)?;
        compute_signing_root(header, domain)
    }

    fn age(&self, slot: u64) -> Duration {
        let expected_time = self.slot_timestamp(slot);
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let delay = now - std::time::Duration::from_secs(expected_time);
        chrono::Duration::from_std(delay).unwrap()
    }

    fn expected_current_slot(&self) -> u64 {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let genesis_time = self.config.chain.genesis_time;
        let since_genesis = now - std::time::Duration::from_secs(genesis_time);

        since_genesis.as_secs() / 12
    }

    fn slot_timestamp(&self, slot: u64) -> u64 {
        slot * 12 + self.config.chain.genesis_time
    }

    // Determines blockhash_slot age and returns true if it is less than 14 days old
    fn is_valid_checkpoint(&self, blockhash_slot: u64) -> bool {
        let current_slot = self.expected_current_slot();
        let current_slot_timestamp = self.slot_timestamp(current_slot);
        let blockhash_slot_timestamp = self.slot_timestamp(blockhash_slot);

        let slot_age = current_slot_timestamp
            .checked_sub(blockhash_slot_timestamp)
            .unwrap_or_default();

        slot_age < self.config.max_checkpoint_age
    }
}

fn get_participating_keys(
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

fn get_bits(bitfield: &Bitvector<512>) -> u64 {
    let mut count = 0;
    bitfield.iter().for_each(|bit| {
        if bit == true {
            count += 1;
        }
    });

    count
}

fn is_finality_proof_valid(
    attested_header: &Header,
    finality_header: &mut Header,
    finality_branch: &[Bytes32],
) -> bool {
    is_proof_valid(attested_header, finality_header, finality_branch, 6, 41)
}

fn is_next_committee_proof_valid(
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

fn is_current_committee_proof_valid(
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::{
        consensus::calc_sync_period,
        constants::MAX_REQUEST_LIGHT_CLIENT_UPDATES,
        errors::ConsensusError,
        rpc::{mock_rpc::MockRpc, ConsensusRpc},
        types::Header,
        types::{BLSPubKey, SignatureBytes},
        Inner,
    };

    use config::{networks, Config};
    use tokio::sync::{mpsc::channel, watch};

    async fn get_client(strict_checkpoint_age: bool, sync: bool) -> Inner<MockRpc> {
        let base_config = networks::mainnet();
        let config = Config {
            consensus_rpc: String::new(),
            execution_rpc: String::new(),
            chain: base_config.chain,
            forks: base_config.forks,
            strict_checkpoint_age,
            ..Default::default()
        };

        let checkpoint =
            hex::decode("5afc212a7924789b2bc86acad3ab3a6ffb1f6e97253ea50bee7f4f51422c9275")
                .unwrap();

        let (block_send, _) = channel(256);
        let (finalized_block_send, _) = watch::channel(None);
        let (channel_send, _) = watch::channel(None);

        let mut client = Inner::new(
            "testdata/",
            block_send,
            finalized_block_send,
            channel_send,
            Arc::new(config),
        );

        if sync {
            client.sync(&checkpoint).await.unwrap()
        } else {
            client.bootstrap(&checkpoint).await.unwrap();
        }

        client
    }

    #[tokio::test]
    async fn test_verify_update() {
        let client = get_client(false, false).await;
        let period = calc_sync_period(client.store.finalized_header.slot.into());
        let updates = client
            .rpc
            .get_updates(period, MAX_REQUEST_LIGHT_CLIENT_UPDATES)
            .await
            .unwrap();

        let update = updates[0].clone();
        client.verify_update(&update).unwrap();
    }

    #[tokio::test]
    async fn test_verify_update_invalid_committee() {
        let client = get_client(false, false).await;
        let period = calc_sync_period(client.store.finalized_header.slot.into());
        let updates = client
            .rpc
            .get_updates(period, MAX_REQUEST_LIGHT_CLIENT_UPDATES)
            .await
            .unwrap();

        let mut update = updates[0].clone();
        update.next_sync_committee.pubkeys[0] = BLSPubKey::default();

        let err = client.verify_update(&update).err().unwrap();
        assert_eq!(
            err.to_string(),
            ConsensusError::InvalidNextSyncCommitteeProof.to_string()
        );
    }

    #[tokio::test]
    async fn test_verify_update_invalid_finality() {
        let client = get_client(false, false).await;
        let period = calc_sync_period(client.store.finalized_header.slot.into());
        let updates = client
            .rpc
            .get_updates(period, MAX_REQUEST_LIGHT_CLIENT_UPDATES)
            .await
            .unwrap();

        let mut update = updates[0].clone();
        update.finalized_header = Header::default();

        let err = client.verify_update(&update).err().unwrap();
        assert_eq!(
            err.to_string(),
            ConsensusError::InvalidFinalityProof.to_string()
        );
    }

    #[tokio::test]
    async fn test_verify_update_invalid_sig() {
        let client = get_client(false, false).await;
        let period = calc_sync_period(client.store.finalized_header.slot.into());
        let updates = client
            .rpc
            .get_updates(period, MAX_REQUEST_LIGHT_CLIENT_UPDATES)
            .await
            .unwrap();

        let mut update = updates[0].clone();
        update.sync_aggregate.sync_committee_signature = SignatureBytes::default();

        let err = client.verify_update(&update).err().unwrap();
        assert_eq!(
            err.to_string(),
            ConsensusError::InvalidSignature.to_string()
        );
    }

    #[tokio::test]
    async fn test_verify_finality() {
        let client = get_client(false, true).await;

        let update = client.rpc.get_finality_update().await.unwrap();

        client.verify_finality_update(&update).unwrap();
    }

    #[tokio::test]
    async fn test_verify_finality_invalid_finality() {
        let client = get_client(false, true).await;

        let mut update = client.rpc.get_finality_update().await.unwrap();
        update.finalized_header = Header::default();

        let err = client.verify_finality_update(&update).err().unwrap();
        assert_eq!(
            err.to_string(),
            ConsensusError::InvalidFinalityProof.to_string()
        );
    }

    #[tokio::test]
    async fn test_verify_finality_invalid_sig() {
        let client = get_client(false, true).await;

        let mut update = client.rpc.get_finality_update().await.unwrap();
        update.sync_aggregate.sync_committee_signature = SignatureBytes::default();

        let err = client.verify_finality_update(&update).err().unwrap();
        assert_eq!(
            err.to_string(),
            ConsensusError::InvalidSignature.to_string()
        );
    }

    #[tokio::test]
    async fn test_verify_optimistic() {
        let client = get_client(false, true).await;

        let update = client.rpc.get_optimistic_update().await.unwrap();
        client.verify_optimistic_update(&update).unwrap();
    }

    #[tokio::test]
    async fn test_verify_optimistic_invalid_sig() {
        let client = get_client(false, true).await;

        let mut update = client.rpc.get_optimistic_update().await.unwrap();
        update.sync_aggregate.sync_committee_signature = SignatureBytes::default();

        let err = client.verify_optimistic_update(&update).err().unwrap();
        assert_eq!(
            err.to_string(),
            ConsensusError::InvalidSignature.to_string()
        );
    }

    #[tokio::test]
    #[should_panic]
    async fn test_verify_checkpoint_age_invalid() {
        get_client(true, false).await;
    }
}
