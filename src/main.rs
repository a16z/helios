use eyre::Result;
use serde::Deserializer;
use ssz_rs::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {

    let mut client = LightClient::new(
        "http://testing.prater.beacon-api.nimbus.team",
        "0x29d7ba1ef23b01a8b9024ee0cd73d0b7181edc0eb16e4645300092838c07783f"
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
        let next_period = current_period +  1;

        let updates = self.get_updates(next_period).await?;

        for mut update in updates {
            self.proccess_update(&mut update)?;
            return Ok(());
        }

        Ok(())
    }

    fn proccess_update(&mut self, update: &mut Update) -> Result<()> {
        let current_slot = self.store.header.slot;
        let update_slot = update.attested_header.slot;
        
        let current_period = calc_sync_period(current_slot);
        let update_period = calc_sync_period(update_slot);

        println!("current period: {}", current_period);
        println!("update period: {}", update_period);

        if !(update_period == current_period + 1) {
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

        let next_committee_hash = update.next_sync_committee.hash_tree_root()?;
        let next_committee_branch = branch_to_nodes(update.next_sync_committee_branch.clone());
        let next_committee_branch_valid = is_valid_merkle_branch(&next_committee_hash, next_committee_branch.iter(), 5, 23, &update_header_root);
        println!("next sync committee branch valid: {}", next_committee_branch_valid);

        if !(finality_branch_valid && next_committee_branch_valid) {
            return Err(eyre::eyre!("Invalid Update"));
        }


        Ok(())
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
    next_sync_committee: SyncCommittee,
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
    finality_branch: Vec<String>,
    sync_aggregate: SyncAggregate,
    signature_slot: String,
}

