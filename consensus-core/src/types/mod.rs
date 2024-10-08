use alloy::primitives::{Address, FixedBytes, B256, U256};
use eyre::Result;
use serde::{Deserialize, Serialize};
use ssz_derive::Encode;
use ssz_types::{serde_utils::quoted_u64_var_list, BitList, BitVector, FixedVector, VariableList};
use superstruct::superstruct;
use tree_hash_derive::TreeHash;

use self::{
    bls::{PublicKey, Signature},
    bytes::{ByteList, ByteVector},
};

pub mod bls;
pub mod bytes;
mod serde_utils;

pub type LogsBloom = ByteVector<typenum::U256>;
pub type KZGCommitment = ByteVector<typenum::U48>;
pub type Transaction = ByteList<typenum::U1073741824>;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LightClientStore {
    pub finalized_header: LightClientHeader,
    pub current_sync_committee: SyncCommittee,
    pub next_sync_committee: Option<SyncCommittee>,
    pub optimistic_header: LightClientHeader,
    pub previous_max_active_participants: u64,
    pub current_max_active_participants: u64,
}

#[derive(Deserialize, Debug, Default, Encode, TreeHash, Clone)]
pub struct BeaconBlock {
    #[serde(with = "serde_utils::u64")]
    pub slot: u64,
    #[serde(with = "serde_utils::u64")]
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
    randao_reveal: Signature,
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
    signature: Signature,
}

#[derive(Default, Clone, Debug, Encode, TreeHash, Deserialize)]
pub struct BlsToExecutionChange {
    #[serde(with = "serde_utils::u64")]
    validator_index: u64,
    from_bls_pubkey: PublicKey,
    to_execution_address: Address,
}

#[superstruct(
    variants(Bellatrix, Capella, Deneb),
    variant_attributes(
        derive(Deserialize, Debug, Default, Encode, TreeHash, Clone),
        serde(deny_unknown_fields),
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
    #[serde(with = "serde_utils::u64")]
    pub block_number: u64,
    #[serde(with = "serde_utils::u64")]
    pub gas_limit: u64,
    #[serde(with = "serde_utils::u64")]
    pub gas_used: u64,
    #[serde(with = "serde_utils::u64")]
    pub timestamp: u64,
    pub extra_data: ByteList<typenum::U32>,
    #[serde(with = "serde_utils::u256")]
    pub base_fee_per_gas: U256,
    pub block_hash: B256,
    pub transactions: VariableList<Transaction, typenum::U1048576>,
    #[superstruct(only(Capella, Deneb))]
    pub withdrawals: VariableList<Withdrawal, typenum::U16>,
    #[superstruct(only(Deneb))]
    #[serde(with = "serde_utils::u64")]
    pub blob_gas_used: u64,
    #[superstruct(only(Deneb))]
    #[serde(with = "serde_utils::u64")]
    pub excess_blob_gas: u64,
}

impl Default for ExecutionPayload {
    fn default() -> Self {
        ExecutionPayload::Bellatrix(ExecutionPayloadBellatrix::default())
    }
}

#[superstruct(
    variants(Bellatrix, Capella, Deneb),
    variant_attributes(
        derive(Serialize, Deserialize, Debug, Default, Encode, TreeHash, Clone),
        serde(deny_unknown_fields),
    )
)]
#[derive(Debug, Clone, Serialize, Deserialize, Encode, TreeHash)]
#[serde(untagged)]
#[ssz(enum_behaviour = "transparent")]
#[tree_hash(enum_behaviour = "transparent")]
pub struct ExecutionPayloadHeader {
    pub parent_hash: B256,
    pub fee_recipient: Address,
    pub state_root: B256,
    pub receipts_root: B256,
    pub logs_bloom: LogsBloom,
    pub prev_randao: B256,
    #[serde(with = "serde_utils::u64")]
    pub block_number: u64,
    #[serde(with = "serde_utils::u64")]
    pub gas_limit: u64,
    #[serde(with = "serde_utils::u64")]
    pub gas_used: u64,
    #[serde(with = "serde_utils::u64")]
    pub timestamp: u64,
    pub extra_data: ByteList<typenum::U32>,
    #[serde(with = "serde_utils::u256")]
    pub base_fee_per_gas: U256,
    pub block_hash: B256,
    pub transactions_root: B256,
    #[superstruct(only(Capella, Deneb))]
    pub withdrawals_root: B256,
    #[superstruct(only(Deneb))]
    #[serde(with = "serde_utils::u64")]
    pub blob_gas_used: u64,
    #[superstruct(only(Deneb))]
    #[serde(with = "serde_utils::u64")]
    pub excess_blob_gas: u64,
}

impl Default for ExecutionPayloadHeader {
    fn default() -> Self {
        ExecutionPayloadHeader::Bellatrix(ExecutionPayloadHeaderBellatrix::default())
    }
}

#[derive(Default, Clone, Debug, Encode, TreeHash, Deserialize)]
pub struct Withdrawal {
    #[serde(with = "serde_utils::u64")]
    index: u64,
    #[serde(with = "serde_utils::u64")]
    validator_index: u64,
    address: Address,
    #[serde(with = "serde_utils::u64")]
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
    signature: Signature,
}

#[derive(Deserialize, Debug, Default, Encode, TreeHash, Clone)]
struct BeaconBlockHeader {
    #[serde(with = "serde_utils::u64")]
    slot: u64,
    #[serde(with = "serde_utils::u64")]
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

#[derive(Deserialize, Debug, Default, Encode, TreeHash, Clone)]
struct IndexedAttestation {
    #[serde(with = "quoted_u64_var_list")]
    attesting_indices: VariableList<u64, typenum::U2048>,
    data: AttestationData,
    signature: Signature,
}

#[derive(Deserialize, Debug, Encode, TreeHash, Clone)]
pub struct Attestation {
    aggregation_bits: BitList<typenum::U2048>,
    data: AttestationData,
    signature: Signature,
}

#[derive(Deserialize, Debug, Default, Encode, TreeHash, Clone)]
struct AttestationData {
    #[serde(with = "serde_utils::u64")]
    slot: u64,
    #[serde(with = "serde_utils::u64")]
    index: u64,
    beacon_block_root: B256,
    source: Checkpoint,
    target: Checkpoint,
}

#[derive(Deserialize, Debug, Default, Encode, TreeHash, Clone)]
struct Checkpoint {
    #[serde(with = "serde_utils::u64")]
    epoch: u64,
    root: B256,
}

#[derive(Deserialize, Debug, Default, Encode, TreeHash, Clone)]
pub struct SignedVoluntaryExit {
    message: VoluntaryExit,
    signature: Signature,
}

#[derive(Deserialize, Debug, Default, Encode, TreeHash, Clone)]
struct VoluntaryExit {
    #[serde(with = "serde_utils::u64")]
    epoch: u64,
    #[serde(with = "serde_utils::u64")]
    validator_index: u64,
}

#[derive(Deserialize, Debug, Default, Encode, TreeHash, Clone)]
pub struct Deposit {
    proof: FixedVector<B256, typenum::U33>,
    data: DepositData,
}

#[derive(Deserialize, Default, Debug, Encode, TreeHash, Clone)]
struct DepositData {
    pubkey: PublicKey,
    withdrawal_credentials: B256,
    #[serde(with = "serde_utils::u64")]
    amount: u64,
    signature: Signature,
}

#[derive(Deserialize, Debug, Default, Encode, TreeHash, Clone)]
pub struct Eth1Data {
    deposit_root: B256,
    #[serde(with = "serde_utils::u64")]
    deposit_count: u64,
    block_hash: B256,
}

#[derive(Deserialize, Debug)]
pub struct Bootstrap {
    pub header: LightClientHeader,
    pub current_sync_committee: SyncCommittee,
    pub current_sync_committee_branch: Vec<B256>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Update {
    pub attested_header: LightClientHeader,
    pub next_sync_committee: SyncCommittee,
    pub next_sync_committee_branch: Vec<B256>,
    pub finalized_header: LightClientHeader,
    pub finality_branch: Vec<B256>,
    pub sync_aggregate: SyncAggregate,
    #[serde(with = "serde_utils::u64")]
    pub signature_slot: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FinalityUpdate {
    pub attested_header: LightClientHeader,
    pub finalized_header: LightClientHeader,
    pub finality_branch: Vec<B256>,
    pub sync_aggregate: SyncAggregate,
    #[serde(with = "serde_utils::u64")]
    pub signature_slot: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OptimisticUpdate {
    pub attested_header: LightClientHeader,
    pub sync_aggregate: SyncAggregate,
    #[serde(with = "serde_utils::u64")]
    pub signature_slot: u64,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LightClientHeader {
    pub beacon: Header,
    pub execution: Option<ExecutionPayloadHeader>,
    pub execution_branch: Option<Vec<B256>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, Encode, TreeHash)]
pub struct Header {
    #[serde(with = "serde_utils::u64")]
    pub slot: u64,
    #[serde(with = "serde_utils::u64")]
    pub proposer_index: u64,
    pub parent_root: B256,
    pub state_root: B256,
    pub body_root: B256,
}

#[derive(Debug, Clone, Default, Encode, TreeHash, Serialize, Deserialize)]
pub struct SyncCommittee {
    pub pubkeys: FixedVector<PublicKey, typenum::U512>,
    pub aggregate_pubkey: PublicKey,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, Encode, TreeHash)]
pub struct SyncAggregate {
    pub sync_committee_bits: BitVector<typenum::U512>,
    pub sync_committee_signature: Signature,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Forks {
    pub genesis: Fork,
    pub altair: Fork,
    pub bellatrix: Fork,
    pub capella: Fork,
    pub deneb: Fork,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Fork {
    pub epoch: u64,
    pub fork_version: FixedBytes<4>,
}

pub struct GenericUpdate {
    pub attested_header: LightClientHeader,
    pub sync_aggregate: SyncAggregate,
    pub signature_slot: u64,
    pub next_sync_committee: Option<SyncCommittee>,
    pub next_sync_committee_branch: Option<Vec<B256>>,
    pub finalized_header: Option<LightClientHeader>,
    pub finality_branch: Option<Vec<B256>>,
}

impl From<&Update> for GenericUpdate {
    fn from(update: &Update) -> Self {
        Self {
            attested_header: update.attested_header.clone(),
            sync_aggregate: update.sync_aggregate.clone(),
            signature_slot: update.signature_slot,
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
            signature_slot: update.signature_slot,
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
            signature_slot: update.signature_slot,
            next_sync_committee: None,
            next_sync_committee_branch: None,
            finalized_header: None,
            finality_branch: None,
        }
    }
}
