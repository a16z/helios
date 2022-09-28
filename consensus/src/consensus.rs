use std::cmp;
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use blst::min_pk::{PublicKey, Signature};
use blst::BLST_ERROR;
use chrono::Duration;
use eyre::{eyre, Result};
use log::{debug, info};
use ssz_rs::prelude::*;

use common::types::*;
use common::utils::*;
use config::Config;

use super::rpc::Rpc;
use super::types::*;

pub struct ConsensusClient<R: Rpc> {
    rpc: R,
    store: Store,
    pub last_checkpoint: Option<Vec<u8>>,
    pub config: Arc<Config>,
}

#[derive(Debug)]
struct Store {
    finalized_header: Header,
    current_sync_committee: SyncCommittee,
    next_sync_committee: Option<SyncCommittee>,
    optimistic_header: Header,
    previous_max_active_participants: u64,
    current_max_active_participants: u64,
}

impl<R: Rpc> ConsensusClient<R> {
    pub async fn new(
        rpc: &str,
        checkpoint_block_root: &Vec<u8>,
        config: Arc<Config>,
    ) -> Result<ConsensusClient<R>> {
        let rpc = R::new(rpc);

        let mut bootstrap = rpc.get_bootstrap(checkpoint_block_root).await?;

        let committee_valid = is_current_committee_proof_valid(
            &bootstrap.header,
            &mut bootstrap.current_sync_committee,
            &bootstrap.current_sync_committee_branch,
        );

        let header_hash = bootstrap.header.hash_tree_root()?;
        let header_valid =
            header_hash.to_string() == format!("0x{}", hex::encode(checkpoint_block_root));

        if !(header_valid && committee_valid) {
            return Err(eyre!("Invalid Bootstrap"));
        }

        let store = Store {
            finalized_header: bootstrap.header.clone(),
            current_sync_committee: bootstrap.current_sync_committee,
            next_sync_committee: None,
            optimistic_header: bootstrap.header.clone(),
            previous_max_active_participants: 0,
            current_max_active_participants: 0,
        };

        Ok(ConsensusClient {
            rpc,
            store,
            last_checkpoint: None,
            config,
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
            return Err(eyre!("Block Not Found"));
        };

        if verified_block_hash != block_hash {
            Err(eyre!("Block Root Mismatch"))
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
        let current_period = calc_sync_period(self.store.finalized_header.slot);
        let updates = self.rpc.get_updates(current_period).await?;

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
            let mut updates = self.rpc.get_updates(current_period).await?;

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

    fn verify_generic_update(&self, update: &GenericUpdate) -> Result<()> {
        let bits = get_bits(&update.sync_aggregate.sync_committee_bits);
        if bits == 0 {
            return Err(eyre!("Insufficient Participation"));
        }

        let update_finalized_slot = update.finalized_header.clone().unwrap_or_default().slot;
        let valid_time = self.current_slot() >= update.signature_slot
            && update.signature_slot > update.attested_header.slot
            && update.attested_header.slot >= update_finalized_slot;

        if !valid_time {
            return Err(eyre!("Invalid Timestamp"));
        }

        let store_period = calc_sync_period(self.store.finalized_header.slot);
        let update_sig_period = calc_sync_period(update.signature_slot);
        let valid_period = if self.store.next_sync_committee.is_some() {
            update_sig_period == store_period || update_sig_period == store_period + 1
        } else {
            update_sig_period == store_period
        };

        if !valid_period {
            return Err(eyre!("Invalid Period"));
        }

        let update_attested_period = calc_sync_period(update.attested_header.slot);
        let update_has_next_committee = self.store.next_sync_committee.is_none()
            && update.next_sync_committee.is_some()
            && update_attested_period == store_period;

        if update.attested_header.slot <= self.store.finalized_header.slot
            && !update_has_next_committee
        {
            return Err(eyre!("Update Not Relevent"));
        }

        if update.finalized_header.is_some() && update.finality_branch.is_some() {
            let is_valid = is_finality_proof_valid(
                &update.attested_header,
                &mut update.finalized_header.clone().unwrap(),
                &update.finality_branch.clone().unwrap(),
            );

            if !is_valid {
                return Err(eyre!("Invalid Finality Proof"));
            }
        }

        if update.next_sync_committee.is_some() && update.next_sync_committee_branch.is_some() {
            let is_valid = is_next_committee_proof_valid(
                &update.attested_header,
                &mut update.next_sync_committee.clone().unwrap(),
                &update.next_sync_committee_branch.clone().unwrap(),
            );

            if !is_valid {
                return Err(eyre!("Invalid Next Sync Committee Proof"));
            }
        }

        let sync_committee = if update_sig_period == store_period {
            &self.store.current_sync_committee
        } else {
            self.store.next_sync_committee.as_ref().unwrap()
        };

        let pks =
            get_participating_keys(sync_committee, &update.sync_aggregate.sync_committee_bits)?;
        let pks: Vec<&PublicKey> = pks.iter().map(|pk| pk).collect();

        let header_root =
            bytes_to_bytes32(update.attested_header.clone().hash_tree_root()?.as_bytes());
        let signing_root = self.compute_committee_sign_root(header_root, update.signature_slot)?;
        let sig = &update.sync_aggregate.sync_committee_signature;
        let is_valid_sig = is_aggregate_valid(sig, signing_root.as_bytes(), &pks);

        if !is_valid_sig {
            return Err(eyre!("Invalid Signature"));
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

    fn apply_generic_update(&mut self, update: &GenericUpdate) {
        let committee_bits = get_bits(&update.sync_aggregate.sync_committee_bits);

        self.store.current_max_active_participants =
            u64::max(self.store.current_max_active_participants, committee_bits);

        let safety_theshhold = cmp::max(
            self.store.current_max_active_participants,
            self.store.previous_max_active_participants,
        ) / 2;

        if committee_bits > safety_theshhold
            && update.attested_header.slot > self.store.optimistic_header.slot
        {
            self.store.optimistic_header = update.attested_header.clone();
            self.log_optimistic_update(update);
        }

        let update_finalized_period =
            calc_sync_period(update.finalized_header.clone().unwrap_or_default().slot);
        let update_attested_period = calc_sync_period(update.attested_header.slot);

        let update_has_finalized_next_committee = self.store.next_sync_committee.is_none()
            && update.next_sync_committee.is_some()
            && update.next_sync_committee_branch.is_some()
            && update.finalized_header.is_some()
            && update.finality_branch.is_some()
            && update_finalized_period == update_attested_period;

        let should_apply = committee_bits * 3 >= 512 * 2
            && (update.finalized_header.clone().unwrap_or_default().slot
                > self.store.finalized_header.slot
                || update_has_finalized_next_committee);

        if should_apply {
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

            if update.finalized_header.clone().unwrap_or_default().slot
                > self.store.finalized_header.slot
            {
                self.store.finalized_header = update.finalized_header.clone().unwrap();
                self.log_finality_update(update);

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
        let delay = self.get_delay(self.store.finalized_header.slot);

        info!(
            "finalized slot             slot={}  confidence={:.2}%  delay={:02}:{:02}:{:02}",
            self.store.finalized_header.slot,
            participation,
            delay.num_hours(),
            delay.num_minutes(),
            delay.num_seconds(),
        );
    }

    fn apply_optimistic_update(&mut self, update: &OptimisticUpdate) {
        let update = GenericUpdate::from(update);
        self.apply_generic_update(&update);
    }

    fn log_optimistic_update(&self, update: &GenericUpdate) {
        let participation =
            get_bits(&update.sync_aggregate.sync_committee_bits) as f32 / 512_f32 * 100f32;
        let delay = self.get_delay(self.store.optimistic_header.slot);

        info!(
            "updated head               slot={}  confidence={:.2}%  delay={:02}:{:02}:{:02}",
            self.store.optimistic_header.slot,
            participation,
            delay.num_hours(),
            delay.num_minutes(),
            delay.num_seconds(),
        );
    }

    fn compute_committee_sign_root(&self, header: Bytes32, slot: u64) -> Result<Node> {
        let genesis_root = self
            .config
            .general
            .genesis_root
            .to_vec()
            .try_into()
            .unwrap();

        let domain_type = &hex::decode("07000000")?[..];
        let fork_version = Vector::from_iter(self.config.fork_version(slot));
        let domain = compute_domain(domain_type, fork_version, genesis_root)?;
        compute_signing_root(header, domain)
    }

    fn get_delay(&self, slot: u64) -> Duration {
        let expected_time = slot * 12 + self.config.general.genesis_time;
        let now = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap();
        let delay = now - std::time::Duration::from_secs(expected_time);
        chrono::Duration::from_std(delay).unwrap()
    }

    fn current_slot(&self) -> u64 {
        let now = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap();

        let genesis_time = self.config.general.genesis_time;
        let since_genesis = now - std::time::Duration::from_secs(genesis_time);

        since_genesis.as_secs() / 12
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

fn is_aggregate_valid(sig_bytes: &SignatureBytes, msg: &[u8], pks: &[&PublicKey]) -> bool {
    let dst: &[u8] = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_";
    let sig_res = Signature::from_bytes(&sig_bytes);
    match sig_res {
        Ok(sig) => sig.fast_aggregate_verify(true, msg, dst, &pks) == BLST_ERROR::BLST_SUCCESS,
        Err(_) => false,
    }
}

fn is_finality_proof_valid(
    attested_header: &Header,
    finality_header: &mut Header,
    finality_branch: &Vec<Bytes32>,
) -> bool {
    let finality_header_hash_res = finality_header.hash_tree_root();
    if finality_header_hash_res.is_err() {
        return false;
    }

    let attested_header_state_root_res = bytes32_to_node(&attested_header.state_root);
    if attested_header_state_root_res.is_err() {
        return false;
    }

    let finality_branch_res = branch_to_nodes(finality_branch.clone());
    if finality_branch_res.is_err() {
        return false;
    }

    is_valid_merkle_branch(
        &finality_header_hash_res.unwrap(),
        finality_branch_res.unwrap().iter(),
        6,
        41,
        &attested_header_state_root_res.unwrap(),
    )
}

fn is_next_committee_proof_valid(
    attested_header: &Header,
    next_committee: &mut SyncCommittee,
    next_committee_branch: &Vec<Bytes32>,
) -> bool {
    let next_committee_hash_res = next_committee.hash_tree_root();
    if next_committee_hash_res.is_err() {
        return false;
    }

    let attested_header_state_root_res = bytes32_to_node(&attested_header.state_root);
    if attested_header_state_root_res.is_err() {
        return false;
    }

    let next_committee_branch_res = branch_to_nodes(next_committee_branch.clone());
    if next_committee_branch_res.is_err() {
        return false;
    }

    is_valid_merkle_branch(
        &next_committee_hash_res.unwrap(),
        next_committee_branch_res.unwrap().iter(),
        5,
        23,
        &attested_header_state_root_res.unwrap(),
    )
}

fn is_current_committee_proof_valid(
    attested_header: &Header,
    current_committee: &mut SyncCommittee,
    current_committee_branch: &Vec<Bytes32>,
) -> bool {
    let next_committee_hash_res = current_committee.hash_tree_root();
    if next_committee_hash_res.is_err() {
        return false;
    }

    let attested_header_state_root_res = bytes32_to_node(&attested_header.state_root);
    if attested_header_state_root_res.is_err() {
        return false;
    }

    let next_committee_branch_res = branch_to_nodes(current_committee_branch.clone());
    if next_committee_branch_res.is_err() {
        return false;
    }

    is_valid_merkle_branch(
        &next_committee_hash_res.unwrap(),
        next_committee_branch_res.unwrap().iter(),
        5,
        22,
        &attested_header_state_root_res.unwrap(),
    )
}

fn calc_sync_period(slot: u64) -> u64 {
    let epoch = slot / 32;
    epoch / 256
}

fn branch_to_nodes(branch: Vec<Bytes32>) -> Result<Vec<Node>> {
    branch
        .iter()
        .map(|elem| bytes32_to_node(elem))
        .collect::<Result<Vec<Node>>>()
}

#[derive(SimpleSerialize, Default, Debug)]
struct SigningData {
    object_root: Bytes32,
    domain: Bytes32,
}

#[derive(SimpleSerialize, Default, Debug)]
struct ForkData {
    current_version: Vector<u8, 4>,
    genesis_validator_root: Bytes32,
}

fn compute_signing_root(object_root: Bytes32, domain: Bytes32) -> Result<Node> {
    let mut data = SigningData {
        object_root,
        domain,
    };
    Ok(data.hash_tree_root()?)
}

fn compute_domain(
    domain_type: &[u8],
    fork_version: Vector<u8, 4>,
    genesis_root: Bytes32,
) -> Result<Bytes32> {
    let fork_data_root = compute_fork_data_root(fork_version, genesis_root)?;
    let start = domain_type;
    let end = &fork_data_root.as_bytes()[..28];
    let d = [start, end].concat();
    Ok(d.to_vec().try_into().unwrap())
}

fn compute_fork_data_root(
    current_version: Vector<u8, 4>,
    genesis_validator_root: Bytes32,
) -> Result<Node> {
    let current_version = current_version.try_into()?;
    let mut fork_data = ForkData {
        current_version,
        genesis_validator_root,
    };
    Ok(fork_data.hash_tree_root()?)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use ssz_rs::Vector;

    use crate::{
        consensus::calc_sync_period,
        rpc::{mock_rpc::MockRpc, Rpc},
        types::Header,
        ConsensusClient,
    };
    use config::networks;

    async fn get_client() -> ConsensusClient<MockRpc> {
        ConsensusClient::new(
            "testdata/",
            &networks::goerli().general.checkpoint,
            Arc::new(networks::goerli()),
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn test_verify_update() {
        let mut client = get_client().await;
        let period = calc_sync_period(client.store.finalized_header.slot);
        let updates = client.rpc.get_updates(period).await.unwrap();

        let mut update = updates[0].clone();
        client.verify_update(&mut update).unwrap();
    }

    #[tokio::test]
    async fn test_verify_update_invalid_committee() {
        let mut client = get_client().await;
        let period = calc_sync_period(client.store.finalized_header.slot);
        let updates = client.rpc.get_updates(period).await.unwrap();

        let mut update = updates[0].clone();
        update.next_sync_committee.pubkeys[0] = Vector::default();

        let res = client.verify_update(&mut update);
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_verify_upadate_invlaid_finality() {
        let mut client = get_client().await;
        let period = calc_sync_period(client.store.finalized_header.slot);
        let updates = client.rpc.get_updates(period).await.unwrap();

        let mut update = updates[0].clone();
        update.finalized_header = Header::default();

        let res = client.verify_update(&mut update);
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_verify_update_invalid_sig() {
        let mut client = get_client().await;
        let period = calc_sync_period(client.store.finalized_header.slot);
        let updates = client.rpc.get_updates(period).await.unwrap();

        let mut update = updates[0].clone();
        update.sync_aggregate.sync_committee_signature = Vector::default();

        let res = client.verify_update(&mut update);
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_verify_finality() {
        let mut client = get_client().await;
        client.sync().await.unwrap();

        let update = client.rpc.get_finality_update().await.unwrap();

        client.verify_finality_update(&update).unwrap();
    }

    #[tokio::test]
    async fn test_verify_finality_invlaid_finality() {
        let mut client = get_client().await;
        client.sync().await.unwrap();

        let mut update = client.rpc.get_finality_update().await.unwrap();
        update.finalized_header = Header::default();

        let res = client.verify_finality_update(&update);
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_verify_finality_invlaid_sig() {
        let mut client = get_client().await;
        client.sync().await.unwrap();

        let mut update = client.rpc.get_finality_update().await.unwrap();
        update.sync_aggregate.sync_committee_signature = Vector::default();

        let res = client.verify_finality_update(&update);
        assert!(res.is_err());
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

        let res = client.verify_optimistic_update(&update);
        assert!(res.is_err());
    }
}
