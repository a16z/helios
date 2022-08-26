use blst::min_pk::{PublicKey, Signature};
use blst::BLST_ERROR;
use eyre::Result;
use ssz_rs::prelude::*;

use super::rpc::Rpc;
use super::types::*;
use crate::common::utils::*;

pub struct ConsensusClient {
    rpc: Rpc,
    store: Store,
}

#[derive(Debug)]
struct Store {
    header: Header,
    current_sync_committee: SyncCommittee,
    next_sync_committee: Option<SyncCommittee>,
}

impl ConsensusClient {
    pub async fn new(nimbus_rpc: &str, checkpoint_block_root: &str) -> Result<ConsensusClient> {
        let rpc = Rpc::new(nimbus_rpc);

        let mut bootstrap = rpc.get_bootstrap(checkpoint_block_root).await?;

        let committee_valid = is_current_committee_proof_valid(
            &bootstrap.header,
            &mut bootstrap.current_sync_committee,
            &bootstrap.current_sync_committee_branch,
        );

        let header_hash = bootstrap.header.hash_tree_root()?;
        let header_valid = header_hash.to_string() == checkpoint_block_root.to_string();

        if !(header_valid && committee_valid) {
            return Err(eyre::eyre!("Invalid Bootstrap"));
        }

        let store = Store {
            header: bootstrap.header,
            current_sync_committee: bootstrap.current_sync_committee,
            next_sync_committee: None,
        };

        Ok(ConsensusClient { rpc, store })
    }

    pub async fn get_execution_payload(&self) -> Result<ExecutionPayload> {
        let slot = self.store.header.slot;
        let mut block = self.rpc.get_block(slot).await?.clone();
        let block_hash = block.hash_tree_root()?;
        let verified_block_hash = self.store.header.clone().hash_tree_root()?;

        if verified_block_hash != block_hash {
            Err(eyre::eyre!("Block Root Mismatch"))
        } else {
            Ok(block.body.execution_payload)
        }
    }

    pub fn get_head(&self) -> &Header {
        &self.store.header
    }

    pub async fn sync(&mut self) -> Result<()> {
        let current_period = calc_sync_period(self.store.header.slot);
        let updates = self.rpc.get_updates(current_period).await?;

        for mut update in updates {
            self.verify_update(&mut update)?;
            self.apply_update(&update);
        }

        let finality_update = self.rpc.get_finality_update().await?;
        let mut finality_update_generic = Update {
            attested_header: finality_update.attested_header,
            next_sync_committee: None,
            next_sync_committee_branch: Vec::new(),
            finalized_header: finality_update.finalized_header,
            finality_branch: finality_update.finality_branch,
            sync_aggregate: finality_update.sync_aggregate,
            signature_slot: finality_update.signature_slot,
        };

        self.verify_update(&mut finality_update_generic)?;
        self.apply_update(&finality_update_generic);

        self.rpc.get_block(self.store.header.slot).await?;

        Ok(())
    }

    fn verify_update(&mut self, update: &mut Update) -> Result<()> {
        let current_slot = self.store.header.slot;
        let update_slot = update.finalized_header.slot;

        let current_period = calc_sync_period(current_slot);
        let update_period = calc_sync_period(update_slot);

        if !(update_period == current_period + 1 || update_period == current_period) {
            return Err(eyre::eyre!("Invalid Update"));
        }

        if !(update.signature_slot > update.attested_header.slot
            && update.attested_header.slot > update.finalized_header.slot)
        {
            return Err(eyre::eyre!("Invalid Update"));
        }

        let finality_branch_valid = is_finality_proof_valid(
            &update.attested_header,
            &mut update.finalized_header,
            &update.finality_branch,
        );

        if !(finality_branch_valid) {
            return Err(eyre::eyre!("Invalid Update"));
        }

        if update.next_sync_committee.is_some() {
            let next_committee_branch_valid = is_next_committee_proof_valid(
                &update.attested_header,
                &mut update.next_sync_committee.clone().unwrap(),
                &update.next_sync_committee_branch,
            );

            if !next_committee_branch_valid {
                return Err(eyre::eyre!("Invalid Update"));
            }
        }

        let sync_committee = if current_period == update_period {
            &self.store.current_sync_committee
        } else {
            self.store.next_sync_committee.as_ref().unwrap()
        };

        let pks =
            get_participating_keys(sync_committee, &update.sync_aggregate.sync_committee_bits)?;
        let pks: Vec<&PublicKey> = pks.iter().map(|pk| pk).collect();

        let committee_quorum = pks.len() > 1;
        if !committee_quorum {
            return Err(eyre::eyre!("Invalid Update"));
        }

        let header_root = bytes_to_bytes32(update.attested_header.hash_tree_root()?.as_bytes());
        let signing_root = compute_committee_sign_root(header_root)?;
        let sig = &update.sync_aggregate.sync_committee_signature;
        let is_valid_sig = is_aggregate_valid(sig, signing_root.as_bytes(), &pks);

        if !is_valid_sig {
            return Err(eyre::eyre!("Invalid Update"));
        }

        Ok(())
    }

    fn apply_update(&mut self, update: &Update) {
        let current_period = calc_sync_period(self.store.header.slot);
        let update_period = calc_sync_period(update.finalized_header.slot);

        self.store.header = update.finalized_header.clone();

        if self.store.next_sync_committee.is_none() {
            self.store.next_sync_committee =
                Some(update.next_sync_committee.as_ref().unwrap().clone());
        } else if update_period == current_period + 1 {
            self.store.current_sync_committee =
                self.store.next_sync_committee.as_ref().unwrap().clone();
            self.store.next_sync_committee =
                Some(update.next_sync_committee.as_ref().unwrap().clone());
        }
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

fn compute_committee_sign_root(header: Bytes32) -> Result<Node> {
    let genesis_root =
        hex::decode("043db0d9a83813551ee2f33450d23797757d430911a9320530ad8a0eabc43efb")?
            .to_vec()
            .try_into()
            .unwrap();
    let domain_type = &hex::decode("07000000")?[..];
    let fork_version = Vector::from_iter(hex::decode("02001020").unwrap());
    let domain = compute_domain(domain_type, fork_version, genesis_root)?;
    compute_signing_root(header, domain)
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
