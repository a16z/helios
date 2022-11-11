use std::cmp;
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use blst::min_pk::PublicKey;
use chrono::Duration;
use eyre::eyre;
use eyre::Result;
use log::warn;
use log::{debug, info};
use ssz_rs::prelude::*;

use common::types::*;
use common::utils::*;
use config::Config;

use crate::constants::MAX_REQUEST_LIGHT_CLIENT_UPDATES;
use crate::errors::ConsensusError;

use super::rpc::ConsensusRpc;
use super::types::*;
use super::utils::*;

// https://github.com/ethereum/consensus-specs/blob/dev/specs/altair/light-client/sync-protocol.md
// does not implement force updates

pub struct ConsensusClient<R: ConsensusRpc> {
    rpc: R,
    store: LightClientStore,
    initial_checkpoint: Vec<u8>,
    pub last_checkpoint: Option<Vec<u8>>,
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

impl<R: ConsensusRpc> ConsensusClient<R> {
    pub fn new(
        rpc: &str,
        checkpoint_block_root: &Vec<u8>,
        config: Arc<Config>,
    ) -> Result<ConsensusClient<R>> {
        let rpc = R::new(rpc);

        Ok(ConsensusClient {
            rpc,
            store: LightClientStore::default(),
            last_checkpoint: None,
            config,
            initial_checkpoint: checkpoint_block_root.clone(),
        })
    }

    pub async fn get_execution_payload(&self, slot: &Option<u64>) -> Result<ExecutionPayload> {
        let slot = slot.unwrap_or(self.store.optimistic_header.slot);
        let mut block = self.rpc.get_block(slot).await?.clone();
        let block_hash = block.hash_tree_root()?;

        let latest_slot = self.store.optimistic_header.slot;
        let finalized_slot = self.store.finalized_header.slot;

        let verified_block_hash = if slot == latest_slot {
            self.store.optimistic_header.clone().hash_tree_root()?
        } else if slot == finalized_slot {
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
            Ok(block.body.execution_payload)
        }
    }

    pub fn get_header(&self) -> &Header {
        &self.store.optimistic_header
    }

    pub fn get_finalized_header(&self) -> &Header {
        &self.store.finalized_header
    }

    pub async fn sync(&mut self) -> Result<()> {
        self.bootstrap().await?;

        let current_period = calc_sync_period(self.store.finalized_header.slot);
        let updates = self
            .rpc
            .get_updates(current_period, MAX_REQUEST_LIGHT_CLIENT_UPDATES)
            .await?;

        for mut update in updates {
            self.verify_update(&mut update)?;
            self.apply_update(&update);
        }

        let finality_update = self.rpc.get_finality_update().await?;
        self.verify_finality_update(&finality_update)?;
        self.apply_finality_update(&finality_update);

        let optimistic_update = self.rpc.get_optimistic_update().await?;
        self.verify_optimistic_update(&optimistic_update)?;
        self.apply_optimistic_update(&optimistic_update);

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
            debug!("checking for sync committee update");
            let current_period = calc_sync_period(self.store.finalized_header.slot);
            let mut updates = self.rpc.get_updates(current_period, 1).await?;

            if updates.len() == 1 {
                let mut update = updates.get_mut(0).unwrap();
                let res = self.verify_update(&mut update);

                if res.is_ok() {
                    info!("updating sync committee");
                    self.apply_update(&update);
                }
            }
        }

        Ok(())
    }

    async fn bootstrap(&mut self) -> Result<()> {
        let mut bootstrap = self
            .rpc
            .get_bootstrap(&self.initial_checkpoint)
            .await
            .map_err(|_| eyre!("could not fetch bootstrap"))?;

        let is_valid = self.is_valid_blockhash(bootstrap.header.slot);
        if !is_valid {
            return Err(ConsensusError::InvalidTimestamp.into());
        }

        let committee_valid = is_current_committee_proof_valid(
            &bootstrap.header,
            &mut bootstrap.current_sync_committee,
            &bootstrap.current_sync_committee_branch,
        );

        let header_hash = bootstrap.header.hash_tree_root()?.to_string();
        let expected_hash = format!("0x{}", hex::encode(&self.initial_checkpoint));
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
            && update.signature_slot > update.attested_header.slot
            && update.attested_header.slot >= update_finalized_slot;

        if !valid_time {
            return Err(ConsensusError::InvalidTimestamp.into());
        }

        let store_period = calc_sync_period(self.store.finalized_header.slot);
        let update_sig_period = calc_sync_period(update.signature_slot);
        let valid_period = if self.store.next_sync_committee.is_some() {
            update_sig_period == store_period || update_sig_period == store_period + 1
        } else {
            update_sig_period == store_period
        };

        if !valid_period {
            return Err(ConsensusError::InvalidPeriod.into());
        }

        let update_attested_period = calc_sync_period(update.attested_header.slot);
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

        let update_attested_period = calc_sync_period(update.attested_header.slot);

        let update_finalized_slot = update
            .finalized_header
            .as_ref()
            .map(|h| h.slot)
            .unwrap_or(0);

        let update_finalized_period = calc_sync_period(update_finalized_slot);

        let update_has_finalized_next_committee = self.store.next_sync_committee.is_none()
            && self.has_sync_update(update)
            && self.has_finality_update(update)
            && update_finalized_period == update_attested_period;

        let should_apply_update = {
            let has_majority = committee_bits * 3 >= 512 * 2;
            let update_is_newer = update_finalized_slot > self.store.finalized_header.slot;
            let good_update = update_is_newer || update_has_finalized_next_committee;

            has_majority && good_update
        };

        if should_apply_update {
            let store_period = calc_sync_period(self.store.finalized_header.slot);

            if self.store.next_sync_committee.is_none() {
                self.store.next_sync_committee = update.next_sync_committee.clone();
            } else if update_finalized_period == store_period + 1 {
                info!("sync committee updated");
                self.store.current_sync_committee = self.store.next_sync_committee.clone().unwrap();
                self.store.next_sync_committee = update.next_sync_committee.clone();
                self.store.previous_max_active_participants =
                    self.store.current_max_active_participants;
                self.store.current_max_active_participants = 0;
            }

            if update_finalized_slot > self.store.finalized_header.slot {
                self.store.finalized_header = update.finalized_header.clone().unwrap();
                self.log_finality_update(update);

                if self.store.finalized_header.slot % 32 == 0 {
                    let checkpoint_res = self.store.finalized_header.hash_tree_root();
                    if let Ok(checkpoint) = checkpoint_res {
                        self.last_checkpoint = Some(checkpoint.as_bytes().to_vec());
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
        let age = self.age(self.store.finalized_header.slot);

        info!(
            "finalized slot             slot={}  confidence={:.decimals$}%  age={:02}:{:02}:{:02}:{:02}",
            self.store.finalized_header.slot,
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
        let age = self.age(self.store.optimistic_header.slot);

        info!(
            "updated head               slot={}  confidence={:.decimals$}%  age={:02}:{:02}:{:02}:{:02}",
            self.store.optimistic_header.slot,
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
        pks: &Vec<PublicKey>,
        attested_header: &Header,
        signature: &SignatureBytes,
        signature_slot: u64,
    ) -> bool {
        let res: Result<bool> = (move || {
            let pks: Vec<&PublicKey> = pks.iter().map(|pk| pk).collect();
            let header_root =
                bytes_to_bytes32(attested_header.clone().hash_tree_root()?.as_bytes());
            let signing_root = self.compute_committee_sign_root(header_root, signature_slot)?;

            Ok(is_aggregate_valid(signature, signing_root.as_bytes(), &pks))
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
        let fork_version = Vector::from_iter(self.config.fork_version(slot));
        let domain = compute_domain(domain_type, fork_version, genesis_root)?;
        compute_signing_root(header, domain)
    }

    fn age(&self, slot: u64) -> Duration {
        let expected_time = self.slot_timestamp(slot);
        let now = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap();
        let delay = now - std::time::Duration::from_secs(expected_time);
        chrono::Duration::from_std(delay).unwrap()
    }

    pub fn expected_current_slot(&self) -> u64 {
        let now = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap();

        let genesis_time = self.config.chain.genesis_time;
        let since_genesis = now - std::time::Duration::from_secs(genesis_time);

        since_genesis.as_secs() / 12
    }

    fn slot_timestamp(&self, slot: u64) -> u64 {
        slot * 12 + self.config.chain.genesis_time
    }

    /// Gets the duration until the next update
    /// Updates are scheduled for 4 seconds into each slot
    pub fn duration_until_next_update(&self) -> Duration {
        let current_slot = self.expected_current_slot();
        let next_slot = current_slot + 1;
        let next_slot_timestamp = self.slot_timestamp(next_slot);

        let now = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let time_to_next_slot = next_slot_timestamp - now;
        let next_update = time_to_next_slot + 4;

        Duration::seconds(next_update as i64)
    }

    // Determines blockhash_slot age and returns true if it is less than 14 days old
    fn is_valid_blockhash(&self, blockhash_slot: u64) -> bool {
        let current_slot = self.expected_current_slot();
        let current_slot_timestamp = self.slot_timestamp(current_slot);
        let blockhash_slot_timestamp = self.slot_timestamp(blockhash_slot);

        let slot_age = current_slot_timestamp - blockhash_slot_timestamp;

        slot_age < 14 * 86_400 // this is 14 days in seconds
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
            let pk = PublicKey::from_bytes(&pk).unwrap();
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
    finality_branch: &Vec<Bytes32>,
) -> bool {
    is_proof_valid(attested_header, finality_header, finality_branch, 6, 41)
}

fn is_next_committee_proof_valid(
    attested_header: &Header,
    next_committee: &mut SyncCommittee,
    next_committee_branch: &Vec<Bytes32>,
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
    current_committee_branch: &Vec<Bytes32>,
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

    use crate::constants::MAX_REQUEST_LIGHT_CLIENT_UPDATES;
    use ssz_rs::Vector;

    use crate::{
        consensus::calc_sync_period,
        errors::ConsensusError,
        rpc::{mock_rpc::MockRpc, ConsensusRpc},
        types::Header,
        ConsensusClient,
    };
    use config::{networks, Config};

    async fn get_client() -> ConsensusClient<MockRpc> {
        let base_config = networks::goerli();
        let config = Config {
            consensus_rpc: String::new(),
            execution_rpc: String::new(),
            chain: base_config.chain,
            forks: base_config.forks,
            ..Default::default()
        };

        let checkpoint =
            hex::decode("1e591af1e90f2db918b2a132991c7c2ee9a4ab26da496bd6e71e4f0bd65ea870")
                .unwrap();

        let mut client = ConsensusClient::new("testdata/", &checkpoint, Arc::new(config)).unwrap();
        client.bootstrap().await.unwrap();

        client
    }

    #[tokio::test]
    async fn test_verify_update() {
        let client = get_client().await;
        let period = calc_sync_period(client.store.finalized_header.slot);
        let updates = client
            .rpc
            .get_updates(period, MAX_REQUEST_LIGHT_CLIENT_UPDATES)
            .await
            .unwrap();

        let mut update = updates[0].clone();
        client.verify_update(&mut update).unwrap();
    }

    #[tokio::test]
    async fn test_verify_update_invalid_committee() {
        let client = get_client().await;
        let period = calc_sync_period(client.store.finalized_header.slot);
        let updates = client
            .rpc
            .get_updates(period, MAX_REQUEST_LIGHT_CLIENT_UPDATES)
            .await
            .unwrap();

        let mut update = updates[0].clone();
        update.next_sync_committee.pubkeys[0] = Vector::default();

        let err = client.verify_update(&mut update).err().unwrap();
        assert_eq!(
            err.to_string(),
            ConsensusError::InvalidNextSyncCommitteeProof.to_string()
        );
    }

    #[tokio::test]
    async fn test_verify_update_invalid_finality() {
        let client = get_client().await;
        let period = calc_sync_period(client.store.finalized_header.slot);
        let updates = client
            .rpc
            .get_updates(period, MAX_REQUEST_LIGHT_CLIENT_UPDATES)
            .await
            .unwrap();

        let mut update = updates[0].clone();
        update.finalized_header = Header::default();

        let err = client.verify_update(&mut update).err().unwrap();
        assert_eq!(
            err.to_string(),
            ConsensusError::InvalidFinalityProof.to_string()
        );
    }

    #[tokio::test]
    async fn test_verify_update_invalid_sig() {
        let client = get_client().await;
        let period = calc_sync_period(client.store.finalized_header.slot);
        let updates = client
            .rpc
            .get_updates(period, MAX_REQUEST_LIGHT_CLIENT_UPDATES)
            .await
            .unwrap();

        let mut update = updates[0].clone();
        update.sync_aggregate.sync_committee_signature = Vector::default();

        let err = client.verify_update(&mut update).err().unwrap();
        assert_eq!(
            err.to_string(),
            ConsensusError::InvalidSignature.to_string()
        );
    }

    #[tokio::test]
    async fn test_verify_finality() {
        let mut client = get_client().await;
        client.sync().await.unwrap();

        let update = client.rpc.get_finality_update().await.unwrap();

        client.verify_finality_update(&update).unwrap();
    }

    #[tokio::test]
    async fn test_verify_finality_invalid_finality() {
        let mut client = get_client().await;
        client.sync().await.unwrap();

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
        let mut client = get_client().await;
        client.sync().await.unwrap();

        let mut update = client.rpc.get_finality_update().await.unwrap();
        update.sync_aggregate.sync_committee_signature = Vector::default();

        let err = client.verify_finality_update(&update).err().unwrap();
        assert_eq!(
            err.to_string(),
            ConsensusError::InvalidSignature.to_string()
        );
    }

    #[tokio::test]
    async fn test_verify_optimistic() {
        let mut client = get_client().await;
        client.sync().await.unwrap();

        let update = client.rpc.get_optimistic_update().await.unwrap();
        client.verify_optimistic_update(&update).unwrap();
    }

    #[tokio::test]
    async fn test_verify_optimistic_invalid_sig() {
        let mut client = get_client().await;
        client.sync().await.unwrap();

        let mut update = client.rpc.get_optimistic_update().await.unwrap();
        update.sync_aggregate.sync_committee_signature = Vector::default();

        let err = client.verify_optimistic_update(&update).err().unwrap();
        assert_eq!(
            err.to_string(),
            ConsensusError::InvalidSignature.to_string()
        );
    }
}
