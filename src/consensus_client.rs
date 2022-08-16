use eyre::Result;
use ssz_rs::prelude::*;
use super::utils::*;
use serde::de::Error;

pub struct ConsensusClient {
    rpc: String,
}

impl ConsensusClient {
    pub fn new(rpc: &str) -> Self {
        ConsensusClient { rpc: rpc.to_string() }
    }

    pub async fn get_bootstrap(&self, block_root: &str) -> Result<Bootstrap> {
        let req = format!("{}/eth/v0/beacon/light_client/bootstrap/{}", self.rpc, block_root);
        let res = reqwest::get(req).await?.json::<BootstrapResponse>().await?;
        Ok(res.data.v)
    }

    pub async fn get_updates(&self, period: u64) -> Result<Vec<Update>> {
        let req = format!("{}/eth/v0/beacon/light_client/updates?start_period={}&count=1000", self.rpc, period);
        let res = reqwest::get(req).await?.json::<UpdateResponse>().await?;
        Ok(res.data)
    }

    pub async fn get_finality_update(&self) -> Result<FinalityUpdate> {
        let req = format!("{}/eth/v0/beacon/light_client/finality_update", self.rpc);
        let res = reqwest::get(req).await?.json::<FinalityUpdateResponse>().await?;
        Ok(res.data)
    }
}

pub type BLSPubKey = Vector<u8, 48>;
pub type Bytes32 = Vector<u8, 32>;

#[derive(serde::Deserialize, Debug)]
pub struct Bootstrap {
    pub header: Header,
    pub current_sync_committee: SyncCommittee,
    #[serde(deserialize_with = "branch_deserialize")]
    pub current_sync_committee_branch: Vec<Bytes32>,
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct Update {
    pub attested_header: Header,
    pub next_sync_committee: Option<SyncCommittee>,
    #[serde(deserialize_with = "branch_deserialize")]
    pub next_sync_committee_branch: Vec<Bytes32>,
    pub finalized_header: Header,
    #[serde(deserialize_with = "branch_deserialize")]
    pub finality_branch: Vec<Bytes32>,
    pub sync_aggregate: SyncAggregate,
    #[serde(deserialize_with = "u64_deserialize")]
    pub signature_slot: u64,
}

#[derive(serde::Deserialize, Debug)]
pub struct FinalityUpdate {
    pub attested_header: Header,
    pub finalized_header: Header,
    #[serde(deserialize_with = "branch_deserialize")]
    pub finality_branch: Vec<Bytes32>,
    pub sync_aggregate: SyncAggregate,
    #[serde(deserialize_with = "u64_deserialize")]
    pub signature_slot: u64,
}

#[derive(serde::Deserialize, Debug, Clone, Default, SimpleSerialize)]
pub struct Header {
    #[serde(deserialize_with = "u64_deserialize")]
    pub slot: u64,
    #[serde(deserialize_with = "u64_deserialize")]
    pub proposer_index: u64,
    #[serde(deserialize_with = "bytes32_deserialize")]
    pub parent_root: Bytes32,
    #[serde(deserialize_with = "bytes32_deserialize")]
    pub state_root: Bytes32,
    #[serde(deserialize_with = "bytes32_deserialize")]
    pub body_root: Bytes32,
}

#[derive(Debug, Clone, Default, SimpleSerialize, serde::Deserialize)]
pub struct SyncCommittee {
    #[serde(deserialize_with = "pubkeys_deserialize")]
    pub pubkeys: Vector<BLSPubKey, 512>,
    #[serde(deserialize_with = "pubkey_deserialize")]
    pub aggregate_pubkey: BLSPubKey,
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct SyncAggregate {
    pub sync_committee_bits: String,
    pub sync_committee_signature: String,
}

#[derive(serde::Deserialize, Debug)]
struct UpdateResponse {
    data: Vec<Update>,
}

#[derive(serde::Deserialize, Debug)]
struct FinalityUpdateResponse {
    data: FinalityUpdate,
}

#[derive(serde::Deserialize, Debug)]
struct BootstrapResponse {
    data: BootstrapData,
}

#[derive(serde::Deserialize, Debug)]
struct BootstrapData {
    v: Bootstrap,
}

fn pubkey_deserialize<'de, D>(deserializer: D) -> Result<BLSPubKey, D::Error> where D: serde::Deserializer<'de> {
    let key: String = serde::Deserialize::deserialize(deserializer)?;
    let key_bytes = hex_str_to_bytes(&key).map_err(D::Error::custom)?;
    Ok(Vector::from_iter(key_bytes))
}

fn pubkeys_deserialize<'de, D>(deserializer: D) -> Result<Vector<BLSPubKey, 512>, D::Error> where D: serde::Deserializer<'de> {
    let keys: Vec<String> = serde::Deserialize::deserialize(deserializer)?;
    Ok(keys.iter().map(|key| {
        let key_bytes = hex_str_to_bytes(key)?;
        Ok(Vector::from_iter(key_bytes))
    }).collect::<Result<Vector<BLSPubKey, 512>>>().map_err(D::Error::custom)?)
}

fn branch_deserialize<'de, D>(deserializer: D) -> Result<Vec<Bytes32>, D::Error> where D: serde::Deserializer<'de> {
    let branch: Vec<String> = serde::Deserialize::deserialize(deserializer)?;
    Ok(branch.iter().map(|elem| {
        let elem_bytes = hex_str_to_bytes(elem)?;
        Ok(Vector::from_iter(elem_bytes))
    }).collect::<Result<_>>().map_err(D::Error::custom)?)
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
