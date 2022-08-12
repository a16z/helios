use eyre::Result;
use serde::{Deserialize, Deserializer};
use ssz::Encode;
use ssz_types::typenum::*;
use ssz_derive::Encode;
use ssz_types::FixedVector;
use eth2_hashing::hash;
use merkle_proof::verify_merkle_proof;
use ethereum_types::H256;
use bls::PublicKeyBytes;
use tree_hash_derive::TreeHash;

#[tokio::main]
async fn main() -> Result<()> {

    let client = LightClient::new(
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

        let bootstrap = Self::get_bootstrap(nimbus_rpc, checkpoint_block_root).await?;

        let committee_hash = tree_hash::TreeHash::tree_hash_root(&bootstrap.current_sync_committee);
        println!("{:?}", committee_hash);
        
        let committee_hash = H256::from_slice(&committee_hash[..]);
        let branch = &bootstrap.current_sync_committee_branch.iter().map(|elem| {
            H256::from_slice(&hex::decode(elem.strip_prefix("0x").unwrap()).unwrap())
        }).collect::<Vec<H256>>()[..];
         
         let index = 22;
         let depth = 5;

         let root = H256::from_slice(&hex::decode(checkpoint_block_root.strip_prefix("0x").unwrap()).unwrap());

         let is_valid = verify_merkle_proof(committee_hash, branch, depth, index, root);
         println!("{}", is_valid);
         println!("{:?}", committee_hash);
         println!("{:?}", branch);

        let store = Store {
            header: bootstrap.header,
            current_sync_committee: bootstrap.current_sync_committee,
            next_sync_committee: None,
        };

        Ok(LightClient { nimbus_rpc: nimbus_rpc.to_string(), store })
    }

    async fn sync(&self) -> Result<()> {

        let period = LightClient::calc_sync_period(self.store.header.slot.parse().unwrap());
        let updates = self.get_updates(period).await?;

        for update in updates {
            // TODO: verify update
            println!("{:?}", update.finalized_header);
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

    fn calc_sync_period(slot: u64) -> u64 {
        let epoch = slot / 32;
        epoch / 256
    }
}

#[derive(Deserialize, Debug)]
struct BootstrapResponse {
    data: BootstrapData,
}

#[derive(Deserialize, Debug)]
struct BootstrapData {
    v: Bootstrap,
}

#[derive(Deserialize, Debug)]
struct Bootstrap {
    header: Header,
    current_sync_committee: SyncCommittee,
    current_sync_committee_branch: Vec<String>,
}

#[derive(Deserialize, Debug, Clone, Encode, TreeHash)]
struct SyncCommittee {
    #[serde(deserialize_with = "pubkeys_deserialize")]
    pubkeys: FixedVector<PublicKeyBytes, U512>,
    #[serde(deserialize_with = "pubkey_deserialize")]
    aggregate_pubkey: PublicKeyBytes,
}

fn pubkey_deserialize<'de, D>(deserializer: D) -> Result<PublicKeyBytes, D::Error> where D: Deserializer<'de> {
    let key: String = Deserialize::deserialize(deserializer)?;
    let key_bytes = hex::decode(key.strip_prefix("0x").unwrap()).unwrap();
    Ok(PublicKeyBytes::deserialize(&key_bytes).unwrap())
}

fn pubkeys_deserialize<'de, D>(deserializer: D) -> Result<FixedVector<PublicKeyBytes, U512>, D::Error> where D: Deserializer<'de> {
    let keys: Vec<String> = Deserialize::deserialize(deserializer)?;
    let vec = keys.iter().map(|key| {
        let key_bytes = hex::decode(key.strip_prefix("0x").unwrap()).unwrap();
        PublicKeyBytes::deserialize(&key_bytes).unwrap()
    }).collect::<Vec<PublicKeyBytes>>();
    Ok(FixedVector::new(vec).unwrap())
}

#[derive(Deserialize, Debug, Clone)]
struct Header {
    slot: String,
}

#[derive(Deserialize, Debug)]
struct UpdateResponse {
    data: Vec<Update>,
}

#[derive(Deserialize, Debug, Clone)]
struct Update {
    attested_header: Header,
    next_sync_committee: SyncCommittee,
    next_sync_committee_branch: Vec<String>,
    finalized_header: Header,
    finality_branch: Vec<String>,
    sync_aggregate: SyncAggregate,
}

#[derive(Deserialize, Debug, Clone)]
struct SyncAggregate {
    sync_committee_bits: String,
    sync_committee_signature: String,
}

#[derive(Deserialize, Debug)]
struct FinalityUpdateResponse {
    data: FinalityUpdate,
}

#[derive(Deserialize, Debug)]
struct FinalityUpdate {
    attested_header: Header,
    finalized_header: Header,
    finality_branch: Vec<String>,
    sync_aggregate: SyncAggregate,
    signature_slot: String,
}
