use eyre::Result;
use ssz_rs::prelude::*;
use blst::{min_pk::*, BLST_ERROR};

#[tokio::main]
async fn main() -> Result<()> {

    let mut client = LightClient::new(
        "http://testing.prater.beacon-api.nimbus.team",
        "0x172128eadf1da46467f4d6a822206698e2d3f957af117dd650954780d680dc99"
    ).await?;    
    
    client.sync().await?;

    Ok(())
}

struct LightClient {
    nimbus_rpc: String,
    store: Store,
}

#[derive(Debug)]
struct Store {
    header: Header,
    current_sync_committee: SyncCommittee,
    next_sync_committee: Option<SyncCommittee>,
}

impl LightClient {
    async fn new(nimbus_rpc: &str, checkpoint_block_root: &str) -> Result<LightClient> {

        let mut bootstrap = Self::get_bootstrap(nimbus_rpc, checkpoint_block_root).await?;

        let committee_hash = bootstrap.current_sync_committee.hash_tree_root()?;
        let root = Node::from_bytes(bootstrap.header.state_root);
        let committee_branch = branch_to_nodes(bootstrap.current_sync_committee_branch);
    
        let committee_valid = is_valid_merkle_branch(&committee_hash, committee_branch.iter(), 5, 22, &root);
        println!("bootstrap committee valid: {}", committee_valid);

        let header_hash = bootstrap.header.hash_tree_root()?;
        let header_valid = header_hash.to_string() == checkpoint_block_root.to_string();
        println!("bootstrap header valid: {}", header_valid);

        if !(header_valid && committee_valid) {
            return Err(eyre::eyre!("Invalid Bootstrap"));
        }

        let store = Store {
            header: bootstrap.header,
            current_sync_committee: bootstrap.current_sync_committee,
            next_sync_committee: None,
        };

        Ok(LightClient { nimbus_rpc: nimbus_rpc.to_string(), store })
    }

    async fn sync(&mut self) -> Result<()> {

        let current_period = calc_sync_period(self.store.header.slot);
        let next_period = current_period +  0;

        let updates = self.get_updates(next_period).await?;

        for mut update in updates {
            self.verify_update(&mut update)?;
            self.apply_update(&update);
            println!("================================");
        }

        let finality_update = self.get_finality_update().await?;
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

        println!("synced up to slot: {}", self.store.header.slot);

        Ok(())
    }

    fn verify_update(&mut self, update: &mut Update) -> Result<()> {
        let current_slot = self.store.header.slot;
        let update_slot = update.finalized_header.slot;
        
        let current_period = calc_sync_period(current_slot);
        let update_period = calc_sync_period(update_slot);

        println!("current period: {}", current_period);
        println!("update period: {}", update_period);

        if !(update_period == current_period + 1 || update_period == current_period) {
            return Err(eyre::eyre!("Invalid Update"));
        }

        if !(update.signature_slot > update.attested_header.slot && update.attested_header.slot > update.finalized_header.slot) {
            return Err(eyre::eyre!("Invalid Update"));
        }

        let finality_header_hash = update.finalized_header.hash_tree_root()?;
        let update_header_root = Node::from_bytes(update.attested_header.state_root);
        let finality_branch = branch_to_nodes(update.finality_branch.clone());
        let finality_branch_valid = is_valid_merkle_branch(&finality_header_hash, finality_branch.iter(), 6, 41, &update_header_root);
        println!("finality branch valid: {}", finality_branch_valid);

        if update.next_sync_committee.is_some() {
            let next_committee_hash = update.next_sync_committee.as_ref().unwrap().clone().hash_tree_root()?;
            let next_committee_branch = branch_to_nodes(update.next_sync_committee_branch.clone());
            let next_committee_branch_valid = is_valid_merkle_branch(&next_committee_hash, next_committee_branch.iter(), 5, 23, &update_header_root);
            println!("next sync committee branch valid: {}", next_committee_branch_valid);

            if !next_committee_branch_valid {
                return Err(eyre::eyre!("Invalid Update"));
            }
        }

        if !(finality_branch_valid) {
            return Err(eyre::eyre!("Invalid Update"));
        }

        let sync_committee = if current_period == update_period {
            &self.store.current_sync_committee
        } else {
            self.store.next_sync_committee.as_ref().unwrap()
        };

        let bytes = hex::decode(update.sync_aggregate.sync_committee_bits.strip_prefix("0x").unwrap())?;
        let mut bits = String::new();
        let mut count = 0;
        for byte in bytes {
            let byte_str = format!("{:08b}", byte);
            byte_str.chars().for_each(|b| if b == '1' { count += 1 });
            bits.push_str(&byte_str.chars().rev().collect::<String>());
        }

        let mut pks: Vec<PublicKey> = Vec::new();
        bits.chars().enumerate().for_each(|(i, bit)| {
            if bit == '1' {
                let pk = sync_committee.pubkeys[i].clone();
                let pk = PublicKey::from_bytes(&pk).unwrap();
                pks.push(pk)
            }
        });
        let pks: Vec<&PublicKey> = pks.iter().map(|pk| pk).collect();

        let committee_quorum = count as f64 > 2.0 / 3.0 * 512.0;
        println!("sync committee quorum: {}", committee_quorum);

        if !committee_quorum {
            return Err(eyre::eyre!("Invalid Update"));
        }

        let header_root = bytes_to_bytes32(update.attested_header.hash_tree_root()?.as_bytes());
        let signing_root = compute_committee_sign_root(header_root)?;

        let sig_bytes = hex::decode(update.sync_aggregate.sync_committee_signature.strip_prefix("0x").unwrap())?;
        let sig = Signature::from_bytes(&sig_bytes).unwrap();
        let dst: &[u8] = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_";
        let is_valid_sig = sig.fast_aggregate_verify(true, signing_root.as_bytes(), dst, &pks) == BLST_ERROR::BLST_SUCCESS;
        println!("committee signature valid: {}", is_valid_sig);

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
            self.store.next_sync_committee = Some(update.next_sync_committee.as_ref().unwrap().clone());
        } else if update_period == current_period + 1 {
            self.store.current_sync_committee = self.store.next_sync_committee.as_ref().unwrap().clone();
            self.store.next_sync_committee = Some(update.next_sync_committee.as_ref().unwrap().clone());
        }
    }

    async fn get_bootstrap(rpc: &str, block_root: &str) -> Result<Bootstrap> {
        let req = format!("{}/eth/v0/beacon/light_client/bootstrap/{}", rpc, block_root);
        let res = reqwest::get(req).await?.json::<BootstrapResponse>().await?;
        Ok(res.data.v)
    }

    async fn get_updates(&self, period: u64) -> Result<Vec<Update>> {
        let req = format!("{}/eth/v0/beacon/light_client/updates?start_period={}&count=1000", self.nimbus_rpc, period);
        let res = reqwest::get(req).await?.json::<UpdateResponse>().await?;
        Ok(res.data)
    }

    async fn get_finality_update(&self) -> Result<FinalityUpdate> {
        let req = format!("{}/eth/v0/beacon/light_client/finality_update", self.nimbus_rpc);
        let res = reqwest::get(req).await?.json::<FinalityUpdateResponse>().await?;
        Ok(res.data)
    }
}

fn calc_sync_period(slot: u64) -> u64 {
    let epoch = slot / 32;
    epoch / 256
}

fn branch_to_nodes(branch: Vec<Bytes32>) -> Vec<Node> {
    branch.iter().map(|elem| Node::from_bytes(*elem)).collect()
}

fn bytes_to_bytes32(bytes: &[u8]) -> [u8; 32] {
    bytes.to_vec().try_into().unwrap()
}

fn compute_committee_sign_root(header: Bytes32) -> Result<Node> {
    let genesis_root = hex::decode("043db0d9a83813551ee2f33450d23797757d430911a9320530ad8a0eabc43efb")?.to_vec().try_into().unwrap();
    let domain_type = &hex::decode("07000000")?[..];
    let fork_version = Vector::from_iter(hex::decode("02001020").unwrap());
    let domain = compute_domain(domain_type, fork_version, genesis_root)?;
    compute_signing_root(header, domain)
}

fn compute_signing_root(object_root: Bytes32, domain: Bytes32) -> Result<Node> {
    let mut data = SigningData { object_root, domain };
    Ok(data.hash_tree_root()?)
}

fn compute_domain(domain_type: &[u8], fork_version: Vector<u8, 4>, genesis_root: Bytes32) -> Result<Bytes32> {
    let fork_data_root = compute_fork_data_root(fork_version, genesis_root)?;
    let start = domain_type;
    let end = &fork_data_root.as_bytes()[..28];
    let d = [start, end].concat();
    Ok(d.to_vec().try_into().unwrap())
}

fn compute_fork_data_root(current_version: Vector<u8, 4>, genesis_validator_root: Bytes32) -> Result<Node> {
    let current_version = current_version.try_into()?;
    let mut fork_data = ForkData { current_version, genesis_validator_root };
    Ok(fork_data.hash_tree_root()?)
}

#[derive(SimpleSerialize, Default, Debug)]
struct ForkData {
    current_version: Vector<u8, 4>,
    genesis_validator_root: Bytes32,
}

#[derive(SimpleSerialize, Default, Debug)]
struct SigningData {
    object_root: Bytes32,
    domain: Bytes32
}

#[derive(serde::Deserialize, Debug)]
struct BootstrapResponse {
    data: BootstrapData,
}

#[derive(serde::Deserialize, Debug)]
struct BootstrapData {
    v: Bootstrap,
}

#[derive(serde::Deserialize, Debug)]
struct Bootstrap {
    header: Header,
    current_sync_committee: SyncCommittee,
    #[serde(deserialize_with = "branch_deserialize")]
    current_sync_committee_branch: Vec<Bytes32>,
}

#[derive(Debug, Clone, Default, SimpleSerialize, serde::Deserialize)]
struct SyncCommittee {
    #[serde(deserialize_with = "pubkeys_deserialize")]
    pubkeys: Vector<BLSPubKey, 512>,
    #[serde(deserialize_with = "pubkey_deserialize")]
    aggregate_pubkey: BLSPubKey,
}

type BLSPubKey = Vector<u8, 48>;
type Bytes32 = [u8; 32];

fn pubkey_deserialize<'de, D>(deserializer: D) -> Result<BLSPubKey, D::Error> where D: serde::Deserializer<'de> {
    let key: String = serde::Deserialize::deserialize(deserializer)?;
    let key_bytes = hex::decode(key.strip_prefix("0x").unwrap()).unwrap();
    Ok(Vector::from_iter(key_bytes))
}

fn pubkeys_deserialize<'de, D>(deserializer: D) -> Result<Vector<BLSPubKey, 512>, D::Error> where D: serde::Deserializer<'de> {
    let keys: Vec<String> = serde::Deserialize::deserialize(deserializer)?;
    Ok(keys.iter().map(|key| {
        let key_bytes = hex::decode(key.strip_prefix("0x").unwrap()).unwrap();
        Vector::from_iter(key_bytes)
    }).collect::<Vector<BLSPubKey, 512>>())
}

fn branch_deserialize<'de, D>(deserializer: D) -> Result<Vec<Bytes32>, D::Error> where D: serde::Deserializer<'de> {
    let branch: Vec<String> = serde::Deserialize::deserialize(deserializer)?;
    Ok(branch.iter().map(|elem| {
        let bytes = hex::decode(elem.strip_prefix("0x").unwrap()).unwrap();
        bytes.to_vec().try_into().unwrap()
    }).collect())
}

#[derive(serde::Deserialize, Debug, Clone, Default, SimpleSerialize)]
struct Header {
    #[serde(deserialize_with = "u64_deserialize")]
    slot: u64,
    #[serde(deserialize_with = "u64_deserialize")]
    proposer_index: u64,
    #[serde(deserialize_with = "bytes32_deserialize")]
    parent_root: Bytes32,
    #[serde(deserialize_with = "bytes32_deserialize")]
    state_root: Bytes32,
    #[serde(deserialize_with = "bytes32_deserialize")]
    body_root: Bytes32,
}

fn u64_deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error> where D: serde::Deserializer<'de> {
    let val: String = serde::Deserialize::deserialize(deserializer)?;
    Ok(val.parse().unwrap())
}

fn bytes32_deserialize<'de, D>(deserializer: D) -> Result<Bytes32, D::Error> where D: serde::Deserializer<'de> {
    let bytes: String = serde::Deserialize::deserialize(deserializer)?;
    let bytes = hex::decode(bytes.strip_prefix("0x").unwrap()).unwrap();
    Ok(bytes.to_vec().try_into().unwrap())
}

#[derive(serde::Deserialize, Debug)]
struct UpdateResponse {
    data: Vec<Update>,
}

#[derive(serde::Deserialize, Debug, Clone)]
struct Update {
    attested_header: Header,
    next_sync_committee: Option<SyncCommittee>,
    #[serde(deserialize_with = "branch_deserialize")]
    next_sync_committee_branch: Vec<Bytes32>,
    finalized_header: Header,
    #[serde(deserialize_with = "branch_deserialize")]
    finality_branch: Vec<Bytes32>,
    sync_aggregate: SyncAggregate,
    #[serde(deserialize_with = "u64_deserialize")]
    signature_slot: u64,
}

#[derive(serde::Deserialize, Debug, Clone)]
struct SyncAggregate {
    sync_committee_bits: String,
    sync_committee_signature: String,
}

#[derive(serde::Deserialize, Debug)]
struct FinalityUpdateResponse {
    data: FinalityUpdate,
}

#[derive(serde::Deserialize, Debug)]
struct FinalityUpdate {
    attested_header: Header,
    finalized_header: Header,
    #[serde(deserialize_with = "branch_deserialize")]
    finality_branch: Vec<Bytes32>,
    sync_aggregate: SyncAggregate,
    #[serde(deserialize_with = "u64_deserialize")]
    signature_slot: u64,
}

