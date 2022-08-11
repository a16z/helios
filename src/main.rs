use eyre::Result;
use serde::Deserialize;

#[tokio::main]
async fn main() -> Result<()> {

    let client = LightClient::new("http://testing.prater.beacon-api.nimbus.team");
    let bootstrap = client.get_bootstrap("0x29d7ba1ef23b01a8b9024ee0cd73d0b7181edc0eb16e4645300092838c07783f").await?;
    
    let current_slot = bootstrap.header.slot;
    let current_sync_period = LightClient::calc_sync_period(current_slot.parse()?);
    let next_sync_period = current_sync_period + 1;

    println!("{}", next_sync_period);

    let updates = client.get_updates(next_sync_period).await?;
    let attested_headers = updates.iter().map(|update| &update.attested_header).collect::<Vec<&Header>>();
    let finalized_headers = updates.iter().map(|update| &update.finalized_header).collect::<Vec<&Header>>();

    println!("{:?}", attested_headers);
    println!("{:?}", finalized_headers);

    let finality_update = client.get_finality_update().await?;

    println!("{:?}", finality_update.attested_header);
    println!("{:?}", finality_update.finalized_header);

    Ok(())
}

struct LightClient {
    nimbus_rpc: String,
}

impl LightClient {
    fn new(nimbus_rpc: &str) -> LightClient {
        LightClient { nimbus_rpc: nimbus_rpc.to_string() }
    }

    async fn get_bootstrap(&self, block_root: &str) -> Result<Bootstrap> {
        let req = format!("{}/eth/v0/beacon/light_client/bootstrap/{}", self.nimbus_rpc, block_root);
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

#[derive(Deserialize, Debug, Clone)]
struct SyncCommittee {
    pubkeys: Vec<String>,
    aggregate_pubkey: String,
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
