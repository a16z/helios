use eyre::Result;
use serde::de::Error;
use ssz_rs::prelude::*;

use common::types::Bytes32;
use common::utils::hex_str_to_bytes;

pub type BLSPubKey = Vector<u8, 48>;
pub type SignatureBytes = Vector<u8, 96>;
pub type Address = Vector<u8, 20>;
pub type LogsBloom = Vector<u8, 256>;
pub type Transaction = List<u8, 1073741824>;

#[derive(serde::Deserialize, Debug, Default, SimpleSerialize, Clone)]
pub struct BeaconBlock {
    #[serde(deserialize_with = "u64_deserialize")]
    pub slot: u64,
    #[serde(deserialize_with = "u64_deserialize")]
    pub proposer_index: u64,
    #[serde(deserialize_with = "bytes32_deserialize")]
    pub parent_root: Bytes32,
    #[serde(deserialize_with = "bytes32_deserialize")]
    pub state_root: Bytes32,
    pub body: BeaconBlockBody,
}

#[derive(serde::Deserialize, Debug, Default, SimpleSerialize, Clone)]
pub struct BeaconBlockBody {
    #[serde(deserialize_with = "signature_deserialize")]
    randao_reveal: SignatureBytes,
    eth1_data: Eth1Data,
    #[serde(deserialize_with = "bytes32_deserialize")]
    graffiti: Bytes32,
    proposer_slashings: List<ProposerSlashing, 16>,
    attester_slashings: List<AttesterSlashing, 2>,
    attestations: List<Attestation, 128>,
    deposits: List<Deposit, 16>,
    voluntary_exits: List<SignedVoluntaryExit, 16>,
    sync_aggregate: SyncAggregate,
    pub execution_payload: ExecutionPayload,
}

#[derive(serde::Deserialize, Debug, Default, SimpleSerialize, Clone)]
pub struct ExecutionPayload {
    #[serde(deserialize_with = "bytes32_deserialize")]
    pub parent_hash: Bytes32,
    #[serde(deserialize_with = "address_deserialize")]
    pub fee_recipient: Address,
    #[serde(deserialize_with = "bytes32_deserialize")]
    pub state_root: Bytes32,
    #[serde(deserialize_with = "bytes32_deserialize")]
    pub receipts_root: Bytes32,
    #[serde(deserialize_with = "logs_bloom_deserialize")]
    pub logs_bloom: Vector<u8, 256>,
    #[serde(deserialize_with = "bytes32_deserialize")]
    pub prev_randao: Bytes32,
    #[serde(deserialize_with = "u64_deserialize")]
    pub block_number: u64,
    #[serde(deserialize_with = "u64_deserialize")]
    pub gas_limit: u64,
    #[serde(deserialize_with = "u64_deserialize")]
    pub gas_used: u64,
    #[serde(deserialize_with = "u64_deserialize")]
    pub timestamp: u64,
    #[serde(deserialize_with = "extra_data_deserialize")]
    pub extra_data: List<u8, 32>,
    #[serde(deserialize_with = "u256_deserialize")]
    pub base_fee_per_gas: U256,
    #[serde(deserialize_with = "bytes32_deserialize")]
    pub block_hash: Bytes32,
    #[serde(deserialize_with = "transactions_deserialize")]
    pub transactions: List<Transaction, 1048576>,
}

#[derive(serde::Deserialize, Debug, Default, SimpleSerialize, Clone)]
struct ProposerSlashing {
    signed_header_1: SignedBeaconBlockHeader,
    signed_header_2: SignedBeaconBlockHeader,
}

#[derive(serde::Deserialize, Debug, Default, SimpleSerialize, Clone)]
struct SignedBeaconBlockHeader {
    message: BeaconBlockHeader,
    #[serde(deserialize_with = "signature_deserialize")]
    signature: SignatureBytes,
}

#[derive(serde::Deserialize, Debug, Default, SimpleSerialize, Clone)]
struct BeaconBlockHeader {
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

#[derive(serde::Deserialize, Debug, Default, SimpleSerialize, Clone)]
struct AttesterSlashing {
    attestation_1: IndexedAttestation,
    attestation_2: IndexedAttestation,
}

#[derive(serde::Deserialize, Debug, Default, SimpleSerialize, Clone)]
struct IndexedAttestation {
    #[serde(deserialize_with = "attesting_indices_deserialize")]
    attesting_indices: List<u64, 2048>,
    data: AttestationData,
    #[serde(deserialize_with = "signature_deserialize")]
    signature: SignatureBytes,
}

#[derive(serde::Deserialize, Debug, Default, SimpleSerialize, Clone)]
struct Attestation {
    aggregation_bits: Bitlist<2048>,
    data: AttestationData,
    #[serde(deserialize_with = "signature_deserialize")]
    signature: SignatureBytes,
}

#[derive(serde::Deserialize, Debug, Default, SimpleSerialize, Clone)]
struct AttestationData {
    #[serde(deserialize_with = "u64_deserialize")]
    slot: u64,
    #[serde(deserialize_with = "u64_deserialize")]
    index: u64,
    #[serde(deserialize_with = "bytes32_deserialize")]
    beacon_block_root: Bytes32,
    source: Checkpoint,
    target: Checkpoint,
}

#[derive(serde::Deserialize, Debug, Default, SimpleSerialize, Clone)]
struct Checkpoint {
    #[serde(deserialize_with = "u64_deserialize")]
    epoch: u64,
    #[serde(deserialize_with = "bytes32_deserialize")]
    root: Bytes32,
}

#[derive(serde::Deserialize, Debug, Default, SimpleSerialize, Clone)]
struct SignedVoluntaryExit {
    message: VoluntaryExit,
    #[serde(deserialize_with = "signature_deserialize")]
    signature: SignatureBytes,
}

#[derive(serde::Deserialize, Debug, Default, SimpleSerialize, Clone)]
struct VoluntaryExit {
    #[serde(deserialize_with = "u64_deserialize")]
    epoch: u64,
    #[serde(deserialize_with = "u64_deserialize")]
    validator_index: u64,
}

#[derive(serde::Deserialize, Debug, Default, SimpleSerialize, Clone)]
struct Deposit {
    #[serde(deserialize_with = "bytes_vector_deserialize")]
    proof: Vector<Bytes32, 33>,
    data: DepositData,
}

#[derive(serde::Deserialize, Default, Debug, SimpleSerialize, Clone)]
struct DepositData {
    #[serde(deserialize_with = "pubkey_deserialize")]
    pubkey: BLSPubKey,
    #[serde(deserialize_with = "bytes32_deserialize")]
    withdrawal_credentials: Bytes32,
    #[serde(deserialize_with = "u64_deserialize")]
    amount: u64,
    #[serde(deserialize_with = "signature_deserialize")]
    signature: SignatureBytes,
}

#[derive(serde::Deserialize, Debug, Default, SimpleSerialize, Clone)]
pub struct Eth1Data {
    #[serde(deserialize_with = "bytes32_deserialize")]
    deposit_root: Bytes32,
    #[serde(deserialize_with = "u64_deserialize")]
    deposit_count: u64,
    #[serde(deserialize_with = "bytes32_deserialize")]
    block_hash: Bytes32,
}

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
    pub next_sync_committee: SyncCommittee,
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

#[derive(serde::Deserialize, Debug)]
pub struct OptimisticUpdate {
    pub attested_header: Header,
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

#[derive(serde::Deserialize, Debug, Clone, Default, SimpleSerialize)]
pub struct SyncAggregate {
    pub sync_committee_bits: Bitvector<512>,
    #[serde(deserialize_with = "signature_deserialize")]
    pub sync_committee_signature: SignatureBytes,
}

fn pubkey_deserialize<'de, D>(deserializer: D) -> Result<BLSPubKey, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let key: String = serde::Deserialize::deserialize(deserializer)?;
    let key_bytes = hex_str_to_bytes(&key).map_err(D::Error::custom)?;
    Ok(Vector::from_iter(key_bytes))
}

fn pubkeys_deserialize<'de, D>(deserializer: D) -> Result<Vector<BLSPubKey, 512>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let keys: Vec<String> = serde::Deserialize::deserialize(deserializer)?;
    Ok(keys
        .iter()
        .map(|key| {
            let key_bytes = hex_str_to_bytes(key)?;
            Ok(Vector::from_iter(key_bytes))
        })
        .collect::<Result<Vector<BLSPubKey, 512>>>()
        .map_err(D::Error::custom)?)
}

fn bytes_vector_deserialize<'de, D>(deserializer: D) -> Result<Vector<Bytes32, 33>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let elems: Vec<String> = serde::Deserialize::deserialize(deserializer)?;
    Ok(elems
        .iter()
        .map(|elem| {
            let elem_bytes = hex_str_to_bytes(elem)?;
            Ok(Vector::from_iter(elem_bytes))
        })
        .collect::<Result<Vector<Bytes32, 33>>>()
        .map_err(D::Error::custom)?)
}

fn signature_deserialize<'de, D>(deserializer: D) -> Result<SignatureBytes, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let sig: String = serde::Deserialize::deserialize(deserializer)?;
    let sig_bytes = hex_str_to_bytes(&sig).map_err(D::Error::custom)?;
    Ok(Vector::from_iter(sig_bytes))
}

fn branch_deserialize<'de, D>(deserializer: D) -> Result<Vec<Bytes32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let branch: Vec<String> = serde::Deserialize::deserialize(deserializer)?;
    Ok(branch
        .iter()
        .map(|elem| {
            let elem_bytes = hex_str_to_bytes(elem)?;
            Ok(Vector::from_iter(elem_bytes))
        })
        .collect::<Result<_>>()
        .map_err(D::Error::custom)?)
}

fn u64_deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let val: String = serde::Deserialize::deserialize(deserializer)?;
    Ok(val.parse().unwrap())
}

fn u256_deserialize<'de, D>(deserializer: D) -> Result<U256, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let val: String = serde::Deserialize::deserialize(deserializer)?;
    let x = ethers::types::U256::from_dec_str(&val).map_err(D::Error::custom)?;
    let mut x_bytes = [0; 32];
    x.to_little_endian(&mut x_bytes);
    Ok(U256::from_bytes_le(x_bytes))
}

fn bytes32_deserialize<'de, D>(deserializer: D) -> Result<Bytes32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let bytes: String = serde::Deserialize::deserialize(deserializer)?;
    let bytes = hex::decode(bytes.strip_prefix("0x").unwrap()).unwrap();
    Ok(bytes.to_vec().try_into().unwrap())
}

fn logs_bloom_deserialize<'de, D>(deserializer: D) -> Result<LogsBloom, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let bytes: String = serde::Deserialize::deserialize(deserializer)?;
    let bytes = hex::decode(bytes.strip_prefix("0x").unwrap()).unwrap();
    Ok(bytes.to_vec().try_into().unwrap())
}

fn address_deserialize<'de, D>(deserializer: D) -> Result<Address, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let bytes: String = serde::Deserialize::deserialize(deserializer)?;
    let bytes = hex::decode(bytes.strip_prefix("0x").unwrap()).unwrap();
    Ok(bytes.to_vec().try_into().unwrap())
}

fn extra_data_deserialize<'de, D>(deserializer: D) -> Result<List<u8, 32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let bytes: String = serde::Deserialize::deserialize(deserializer)?;
    let bytes = hex::decode(bytes.strip_prefix("0x").unwrap()).unwrap();
    Ok(bytes.to_vec().try_into().unwrap())
}

fn transactions_deserialize<'de, D>(deserializer: D) -> Result<List<Transaction, 1048576>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let transactions: Vec<String> = serde::Deserialize::deserialize(deserializer)?;
    let transactions = transactions
        .iter()
        .map(|tx| {
            let tx = hex_str_to_bytes(tx).unwrap();
            let tx: Transaction = List::from_iter(tx);
            tx
        })
        .collect::<List<Transaction, 1048576>>();
    Ok(transactions)
}

fn attesting_indices_deserialize<'de, D>(deserializer: D) -> Result<List<u64, 2048>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let attesting_indices: Vec<String> = serde::Deserialize::deserialize(deserializer)?;
    let attesting_indices = attesting_indices
        .iter()
        .map(|i| i.parse().unwrap())
        .collect::<List<u64, 2048>>();

    Ok(attesting_indices)
}
