use eyre::Result;
use serde::Deserializer;
use ssz_rs::prelude::*;

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

        let mut bootstrap = Self::get_bootstrap(nimbus_rpc, checkpoint_block_root).await?;

        let committee_hash = bootstrap.current_sync_committee.hash_tree_root()?;
        let root = Node::from_bytes(hex::decode(bootstrap.header.state_root.strip_prefix("0x").unwrap()).unwrap().try_into().unwrap());
        let committee_branch = bootstrap.current_sync_committee_branch.iter().map(|elem| {
            Node::from_bytes(hex::decode(elem.strip_prefix("0x").unwrap()).unwrap().try_into().unwrap())
        }).collect::<Vec<_>>();

        println!("{}", committee_hash);
    
        let is_valid = is_valid_merkle_branch(&committee_hash, committee_branch.iter(), 5, 22, &root);
        println!("{}", is_valid);

        // let store = Store {
        //     header: bootstrap.header,
        //     current_sync_committee: bootstrap.current_sync_committee,
        //     next_sync_committee: None,
        // };

        // Ok(LightClient { nimbus_rpc: nimbus_rpc.to_string(), store })

        eyre::bail!("")
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
    current_sync_committee_branch: Vec<String>,
}

#[derive(Debug, Clone, Default, SimpleSerialize, serde::Deserialize)]
struct SyncCommittee {
    #[serde(deserialize_with = "pubkeys_deserialize")]
    pubkeys: Vector<BLSPubKey, 512>,
    #[serde(deserialize_with = "pubkey_deserialize")]
    aggregate_pubkey: BLSPubKey,
}

type BLSPubKey = Vector<u8, 48>;

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

#[derive(serde::Deserialize, Debug, Clone)]
struct Header {
    slot: String,
    state_root: String,
    body_root: String,
}

#[derive(serde::Deserialize, Debug)]
struct UpdateResponse {
    data: Vec<Update>,
}

#[derive(serde::Deserialize, Debug, Clone)]
struct Update {
    attested_header: Header,
    next_sync_committee: SyncCommittee,
    next_sync_committee_branch: Vec<String>,
    finalized_header: Header,
    finality_branch: Vec<String>,
    sync_aggregate: SyncAggregate,
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
