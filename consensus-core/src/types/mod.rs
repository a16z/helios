use eyre::Result;
use ssz_derive::Encode;
use ssz_types::{BitList, BitVector, FixedVector, VariableList};
use superstruct::superstruct;
use alloy::primitives::{Address, B256, U256, Bytes};
use serde::{Serialize, Deserialize};
use serde_with::{serde_as, DisplayFromStr, FromInto};

use tree_hash_derive::TreeHash;
use tree_hash::TreeHash;
use utils::header_deserialize;

pub mod primitives;
mod utils;

pub type LogsBloom = FixedVector<u8, typenum::U256>;
pub type BLSPubKey = FixedVector<u8, typenum::U48>;
pub type KZGCommitment = FixedVector<u8, typenum::U48>;
pub type SignatureBytes = FixedVector<u8, typenum::U96>;
pub type Transaction = VariableList<u8, typenum::U1073741824>;

#[derive(Debug, Default, Clone, Deserialize)]
pub struct LightClientStore {
    pub finalized_header: Header,
    pub current_sync_committee: SyncCommittee,
    pub next_sync_committee: Option<SyncCommittee>,
    pub optimistic_header: Header,
    pub previous_max_active_participants: u64,
    pub current_max_active_participants: u64,
}

#[serde_as]
#[derive(Deserialize, Debug, Default, Encode, TreeHash, Clone)]
pub struct BeaconBlock {
    #[serde_as(as = "DisplayFromStr")]
    pub slot: u64,
    #[serde_as(as = "DisplayFromStr")]
    pub proposer_index: u64,
    pub parent_root: B256,
    pub state_root: B256,
    pub body: BeaconBlockBody,
}

#[superstruct(
    variants(Bellatrix, Capella, Deneb),
    variant_attributes(
        derive(Deserialize, Clone, Debug, Encode, TreeHash, Default),
        serde(deny_unknown_fields)
    )
)]
#[derive(Encode, TreeHash, Deserialize, Debug, Clone)]
#[serde(untagged)]
#[ssz(enum_behaviour = "transparent")]
#[tree_hash(enum_behaviour = "transparent")]
pub struct BeaconBlockBody {
    randao_reveal: SignatureBytes,
    eth1_data: Eth1Data,
    graffiti: B256,
    proposer_slashings: VariableList<ProposerSlashing, typenum::U16>,
    attester_slashings: VariableList<AttesterSlashing, typenum::U2>,
    attestations: VariableList<Attestation, typenum::U128>,
    deposits: VariableList<Deposit, typenum::U16>,
    voluntary_exits: VariableList<SignedVoluntaryExit, typenum::U16>,
    sync_aggregate: SyncAggregate,
    pub execution_payload: ExecutionPayload,
    #[superstruct(only(Capella, Deneb))]
    bls_to_execution_changes: VariableList<SignedBlsToExecutionChange, typenum::U16>,
    #[superstruct(only(Deneb))]
    blob_kzg_commitments: VariableList<KZGCommitment, typenum::U4096>,
}

impl Default for BeaconBlockBody {
    fn default() -> Self {
        BeaconBlockBody::Bellatrix(BeaconBlockBodyBellatrix::default())
    }
}

#[derive(Default, Clone, Debug, Encode, TreeHash, Deserialize)]
pub struct SignedBlsToExecutionChange {
    message: BlsToExecutionChange,
    signature: SignatureBytes,
}

#[serde_as]
#[derive(Default, Clone, Debug, Encode, TreeHash, Deserialize)]
pub struct BlsToExecutionChange {
    #[serde_as(as = "DisplayFromStr")]
    validator_index: u64,
    from_bls_pubkey: BLSPubKey,
    to_execution_address: Address,
}


#[superstruct(
    variants(Bellatrix, Capella, Deneb),
    variant_attributes(
        derive(Deserialize, Debug, Default, Encode, TreeHash, Clone),
        serde(deny_unknown_fields),
        serde_as
    )
)]
#[derive(Debug, Clone, Deserialize, Encode, TreeHash)]
#[serde(untagged)]
#[ssz(enum_behaviour = "transparent")]
#[tree_hash(enum_behaviour = "transparent")]
pub struct ExecutionPayload {
    pub parent_hash: B256,
    pub fee_recipient: Address,
    pub state_root: B256,
    pub receipts_root: B256,
    pub logs_bloom: LogsBloom,
    pub prev_randao: B256,
    #[serde_as(as = "DisplayFromStr")]
    pub block_number: u64,
    #[serde_as(as = "DisplayFromStr")]
    pub gas_limit: u64,
    #[serde_as(as = "DisplayFromStr")]
    pub gas_used: u64,
    #[serde_as(as = "DisplayFromStr")]
    pub timestamp: u64,
    pub extra_data: VariableList<u8, typenum::U32>,
    #[serde_as(as = "DisplayFromStr")]
    pub base_fee_per_gas: U256,
    pub block_hash: B256,
    pub transactions: VariableList<Transaction, typenum::U1048576>,
    #[superstruct(only(Capella, Deneb))]
    withdrawals: VariableList<Withdrawal, typenum::U16>,
    #[superstruct(only(Deneb))]
    #[serde_as(as = "DisplayFromStr")]
    blob_gas_used: u64,
    #[superstruct(only(Deneb))]
    #[serde_as(as = "DisplayFromStr")]
    excess_blob_gas: u64,
}

impl Default for ExecutionPayload {
    fn default() -> Self {
        ExecutionPayload::Bellatrix(ExecutionPayloadBellatrix::default())
    }
}

#[serde_as]
#[derive(Default, Clone, Debug, Encode, TreeHash, Deserialize)]
pub struct Withdrawal {
    #[serde_as(as = "DisplayFromStr")]
    index: u64,
    #[serde_as(as = "DisplayFromStr")]
    validator_index: u64,
    address: Address,
    #[serde_as(as = "DisplayFromStr")]
    amount: u64,
}

#[derive(Deserialize, Debug, Default, Encode, TreeHash, Clone)]
pub struct ProposerSlashing {
    signed_header_1: SignedBeaconBlockHeader,
    signed_header_2: SignedBeaconBlockHeader,
}

#[derive(Deserialize, Debug, Default, Encode, TreeHash, Clone)]
struct SignedBeaconBlockHeader {
    message: BeaconBlockHeader,
    signature: SignatureBytes,
}

#[serde_as]
#[derive(Deserialize, Debug, Default, Encode, TreeHash, Clone)]
struct BeaconBlockHeader {
    #[serde_as(as = "DisplayFromStr")]
    slot: u64,
    #[serde_as(as = "DisplayFromStr")]
    proposer_index: u64,
    parent_root: B256,
    state_root: B256,
    body_root: B256,
}

#[derive(Deserialize, Debug, Default, Encode, TreeHash, Clone)]
pub struct AttesterSlashing {
    attestation_1: IndexedAttestation,
    attestation_2: IndexedAttestation,
}

#[serde_as]
#[derive(Deserialize, Debug, Default, Encode, TreeHash, Clone)]
struct IndexedAttestation {
    // #[serde_as(as = "FromInto<Vec<u64>>")]
    attesting_indices: VariableList<u64, typenum::U2048>,
    data: AttestationData,
    signature: SignatureBytes,
}

#[derive(Deserialize, Debug, Encode, TreeHash, Clone)]
pub struct Attestation {
    aggregation_bits: BitList<typenum::U2048>,
    data: AttestationData,
    signature: SignatureBytes,
}

#[serde_as]
#[derive(Deserialize, Debug, Default, Encode, TreeHash, Clone)]
struct AttestationData {
    #[serde_as(as = "DisplayFromStr")]
    slot: u64,
    #[serde_as(as = "DisplayFromStr")]
    index: u64,
    beacon_block_root: B256,
    source: Checkpoint,
    target: Checkpoint,
}

#[serde_as]
#[derive(Deserialize, Debug, Default, Encode, TreeHash, Clone)]
struct Checkpoint {
    #[serde_as(as = "DisplayFromStr")]
    epoch: u64,
    root: B256,
}

#[derive(Deserialize, Debug, Default, Encode, TreeHash, Clone)]
pub struct SignedVoluntaryExit {
    message: VoluntaryExit,
    signature: SignatureBytes,
}

#[serde_as]
#[derive(Deserialize, Debug, Default, Encode, TreeHash, Clone)]
struct VoluntaryExit {
    #[serde_as(as = "DisplayFromStr")]
    epoch: u64,
    #[serde_as(as = "DisplayFromStr")]
    validator_index: u64,
}

#[derive(Deserialize, Debug, Default, Encode, TreeHash, Clone)]
pub struct Deposit {
    proof: FixedVector<B256, typenum::U33>,
    data: DepositData,
}

#[serde_as]
#[derive(Deserialize, Default, Debug, Encode, TreeHash, Clone)]
struct DepositData {
    pubkey: BLSPubKey,
    withdrawal_credentials: B256,
    #[serde_as(as = "DisplayFromStr")]
    amount: u64,
    signature: SignatureBytes,
}

#[derive(Deserialize, Debug, Default, Encode, TreeHash, Clone)]
pub struct Eth1Data {
    deposit_root: B256,
    deposit_count: u64,
    block_hash: B256,
}

#[derive(Deserialize, Debug)]
pub struct Bootstrap {
    #[serde(deserialize_with = "header_deserialize")]
    pub header: Header,
    pub current_sync_committee: SyncCommittee,
    pub current_sync_committee_branch: Vec<B256>,
}

#[serde_as]
#[derive(Deserialize, Debug, Clone)]
pub struct Update {
    #[serde(deserialize_with = "header_deserialize")]
    pub attested_header: Header,
    pub next_sync_committee: SyncCommittee,
    pub next_sync_committee_branch: Vec<B256>,
    #[serde(deserialize_with = "header_deserialize")]
    pub finalized_header: Header,
    pub finality_branch: Vec<B256>,
    pub sync_aggregate: SyncAggregate,
    #[serde_as(as = "DisplayFromStr")]
    pub signature_slot: u64,
}


#[serde_as]
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct FinalityUpdate {
    #[serde(deserialize_with = "header_deserialize")]
    pub attested_header: Header,
    #[serde(deserialize_with = "header_deserialize")]
    pub finalized_header: Header,
    pub finality_branch: Vec<B256>,
    pub sync_aggregate: SyncAggregate,
    #[serde_as(as = "DisplayFromStr")]
    pub signature_slot: u64,
}

#[serde_as]
#[derive(Deserialize, Debug)]
pub struct OptimisticUpdate {
    #[serde(deserialize_with = "header_deserialize")]
    pub attested_header: Header,
    pub sync_aggregate: SyncAggregate,
    #[serde_as(as = "DisplayFromStr")]
    pub signature_slot: u64,
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone, Default, Encode, TreeHash)]
pub struct Header {
    #[serde_as(as = "DisplayFromStr")]
    pub slot: u64,
    #[serde_as(as = "DisplayFromStr")]
    pub proposer_index: u64,
    pub parent_root: B256,
    pub state_root: B256,
    pub body_root: B256,
}

#[serde_as]
#[derive(Debug, Clone, Default, Encode, TreeHash, Deserialize)]
pub struct SyncCommittee {
    pub pubkeys: FixedVector<BLSPubKey, typenum::U512>,
    #[serde_as(as = "serde_with::hex::Hex")]
    pub aggregate_pubkey: BLSPubKey,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, Encode, TreeHash)]
pub struct SyncAggregate {
    pub sync_committee_bits: BitVector<typenum::U512>,
    pub sync_committee_signature: SignatureBytes,
}

pub struct GenericUpdate {
    pub attested_header: Header,
    pub sync_aggregate: SyncAggregate,
    pub signature_slot: u64,
    pub next_sync_committee: Option<SyncCommittee>,
    pub next_sync_committee_branch: Option<Vec<B256>>,
    pub finalized_header: Option<Header>,
    pub finality_branch: Option<Vec<B256>>,
}

impl From<&Update> for GenericUpdate {
    fn from(update: &Update) -> Self {
        Self {
            attested_header: update.attested_header.clone(),
            sync_aggregate: update.sync_aggregate.clone(),
            signature_slot: update.signature_slot.into(),
            next_sync_committee: Some(update.next_sync_committee.clone()),
            next_sync_committee_branch: Some(update.next_sync_committee_branch.clone()),
            finalized_header: Some(update.finalized_header.clone()),
            finality_branch: Some(update.finality_branch.clone()),
        }
    }
}

impl From<&FinalityUpdate> for GenericUpdate {
    fn from(update: &FinalityUpdate) -> Self {
        Self {
            attested_header: update.attested_header.clone(),
            sync_aggregate: update.sync_aggregate.clone(),
            signature_slot: update.signature_slot.into(),
            next_sync_committee: None,
            next_sync_committee_branch: None,
            finalized_header: Some(update.finalized_header.clone()),
            finality_branch: Some(update.finality_branch.clone()),
        }
    }
}

impl From<&OptimisticUpdate> for GenericUpdate {
    fn from(update: &OptimisticUpdate) -> Self {
        Self {
            attested_header: update.attested_header.clone(),
            sync_aggregate: update.sync_aggregate.clone(),
            signature_slot: update.signature_slot.into(),
            next_sync_committee: None,
            next_sync_committee_branch: None,
            finalized_header: None,
            finality_branch: None,
        }
    }
}
