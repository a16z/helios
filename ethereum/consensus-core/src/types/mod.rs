use std::marker::PhantomData;

use alloy::primitives::{Address, FixedBytes, B256, U256};
use alloy_rlp::RlpEncodable;
use eyre::Result;
use serde::{Deserialize, Serialize};
use ssz_derive::{Decode, Encode};
use ssz_types::{serde_utils::quoted_u64_var_list, BitList, BitVector, FixedVector, VariableList};
use superstruct::superstruct;
use tree_hash_derive::TreeHash;

use crate::consensus_spec::ConsensusSpec;

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
pub struct LightClientStore<S: ConsensusSpec> {
    pub finalized_header: LightClientHeader,
    pub current_sync_committee: SyncCommittee<S>,
    pub next_sync_committee: Option<SyncCommittee<S>>,
    pub optimistic_header: LightClientHeader,
    pub previous_max_active_participants: u64,
    pub current_max_active_participants: u64,
    pub best_valid_update: Option<GenericUpdate<S>>,
}

#[derive(Deserialize, Debug, Default, Encode, TreeHash, Clone)]
#[serde(bound = "S: ConsensusSpec")]
pub struct BeaconBlock<S: ConsensusSpec> {
    #[serde(with = "serde_utils::u64")]
    pub slot: u64,
    #[serde(with = "serde_utils::u64")]
    pub proposer_index: u64,
    pub parent_root: B256,
    pub state_root: B256,
    pub body: BeaconBlockBody<S>,
}

#[superstruct(
    variants(Bellatrix, Capella, Deneb),
    variant_attributes(
        derive(Deserialize, Clone, Debug, Encode, TreeHash, Default),
        serde(deny_unknown_fields),
        serde(bound = "S: ConsensusSpec"),
    )
)]
#[derive(Encode, TreeHash, Deserialize, Debug, Clone)]
#[serde(untagged)]
#[serde(bound = "S: ConsensusSpec")]
#[ssz(enum_behaviour = "transparent")]
#[tree_hash(enum_behaviour = "transparent")]
pub struct BeaconBlockBody<S: ConsensusSpec> {
    randao_reveal: Signature,
    eth1_data: Eth1Data,
    graffiti: B256,
    proposer_slashings: VariableList<ProposerSlashing, S::MaxProposerSlashings>,
    attester_slashings: VariableList<AttesterSlashing<S>, S::MaxAttesterSlashings>,
    attestations: VariableList<Attestation<S>, S::MaxAttestations>,
    deposits: VariableList<Deposit, S::MaxDeposits>,
    voluntary_exits: VariableList<SignedVoluntaryExit, S::MaxVoluntaryExits>,
    sync_aggregate: SyncAggregate<S>,
    pub execution_payload: ExecutionPayload<S>,
    #[superstruct(only(Capella, Deneb))]
    bls_to_execution_changes: VariableList<SignedBlsToExecutionChange, S::MaxBlsToExecutionChanged>,
    #[superstruct(only(Deneb))]
    blob_kzg_commitments: VariableList<KZGCommitment, S::MaxBlobKzgCommitments>,
}

impl<S: ConsensusSpec> Default for BeaconBlockBody<S> {
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
        derive(Default, Debug, Deserialize, Encode, TreeHash, Clone),
        serde(deny_unknown_fields),
        serde(bound = "S: ConsensusSpec"),
    )
)]
#[derive(Debug, Deserialize, Clone, Encode, TreeHash)]
#[serde(untagged)]
#[serde(bound = "S: ConsensusSpec")]
#[ssz(enum_behaviour = "transparent")]
#[tree_hash(enum_behaviour = "transparent")]
pub struct ExecutionPayload<S: ConsensusSpec> {
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
    pub withdrawals: VariableList<Withdrawal, S::MaxWithdrawals>,
    #[superstruct(only(Deneb))]
    #[serde(with = "serde_utils::u64")]
    pub blob_gas_used: u64,
    #[superstruct(only(Deneb))]
    #[serde(with = "serde_utils::u64")]
    pub excess_blob_gas: u64,
    #[ssz(skip_serializing, skip_deserializing)]
    #[tree_hash(skip_hashing)]
    #[serde(skip)]
    phantom: PhantomData<S>,
}

impl<S: ConsensusSpec> Default for ExecutionPayload<S> {
    fn default() -> Self {
        ExecutionPayload::<S>::Bellatrix(ExecutionPayloadBellatrix::<S>::default())
    }
}

#[superstruct(
    variants(Bellatrix, Capella, Deneb),
    variant_attributes(
        derive(
            Serialize,
            Deserialize,
            Debug,
            Default,
            Encode,
            Decode,
            TreeHash,
            Clone,
            PartialEq
        ),
        serde(deny_unknown_fields),
    )
)]
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode, TreeHash, PartialEq)]
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

#[derive(Default, Clone, Debug, Encode, TreeHash, Deserialize, RlpEncodable)]
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

#[derive(Serialize, Deserialize, Debug, Default, Encode, Decode, TreeHash, Clone, PartialEq)]
pub struct BeaconBlockHeader {
    #[serde(with = "serde_utils::u64")]
    pub slot: u64,
    #[serde(with = "serde_utils::u64")]
    pub proposer_index: u64,
    pub parent_root: B256,
    pub state_root: B256,
    pub body_root: B256,
}

#[derive(Deserialize, Debug, Default, Encode, TreeHash, Clone)]
#[serde(bound = "S: ConsensusSpec")]
pub struct AttesterSlashing<S: ConsensusSpec> {
    attestation_1: IndexedAttestation<S>,
    attestation_2: IndexedAttestation<S>,
}

#[derive(Deserialize, Debug, Default, Encode, TreeHash, Clone)]
#[serde(bound = "S: ConsensusSpec")]
struct IndexedAttestation<S: ConsensusSpec> {
    #[serde(with = "quoted_u64_var_list")]
    attesting_indices: VariableList<u64, S::MaxValidatorsPerCommitee>,
    data: AttestationData,
    signature: Signature,
}

#[derive(Deserialize, Debug, Encode, TreeHash, Clone)]
#[serde(bound = "S: ConsensusSpec")]
pub struct Attestation<S: ConsensusSpec> {
    aggregation_bits: BitList<S::MaxValidatorsPerCommitee>,
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

#[derive(Deserialize, Debug, Decode)]
#[serde(bound = "S: ConsensusSpec")]
pub struct Bootstrap<S: ConsensusSpec> {
    pub header: LightClientHeader,
    pub current_sync_committee: SyncCommittee<S>,
    pub current_sync_committee_branch: FixedVector<B256, typenum::U5>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Decode)]
#[serde(bound = "S: ConsensusSpec")]
pub struct Update<S: ConsensusSpec> {
    pub attested_header: LightClientHeader,
    pub next_sync_committee: SyncCommittee<S>,
    pub next_sync_committee_branch: FixedVector<B256, typenum::U5>,
    pub finalized_header: LightClientHeader,
    pub finality_branch: FixedVector<B256, typenum::U6>,
    pub sync_aggregate: SyncAggregate<S>,
    #[serde(with = "serde_utils::u64")]
    pub signature_slot: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Decode)]
#[serde(bound = "S: ConsensusSpec")]
pub struct FinalityUpdate<S: ConsensusSpec> {
    pub attested_header: LightClientHeader,
    pub finalized_header: LightClientHeader,
    pub finality_branch: FixedVector<B256, typenum::U6>,
    pub sync_aggregate: SyncAggregate<S>,
    #[serde(with = "serde_utils::u64")]
    pub signature_slot: u64,
}

#[derive(Serialize, Deserialize, Debug, Decode)]
#[serde(bound = "S: ConsensusSpec")]
pub struct OptimisticUpdate<S: ConsensusSpec> {
    pub attested_header: LightClientHeader,
    pub sync_aggregate: SyncAggregate<S>,
    #[serde(with = "serde_utils::u64")]
    pub signature_slot: u64,
}

#[superstruct(
    variants(Bellatrix, Capella, Deneb),
    variant_attributes(
        derive(Default, Debug, Clone, Serialize, Deserialize, Decode, PartialEq),
        serde(deny_unknown_fields),
    )
)]
#[derive(Debug, Clone, Serialize, Deserialize, Decode, PartialEq)]
#[serde(untagged)]
#[ssz(enum_behaviour = "transparent")]
pub struct LightClientHeader {
    pub beacon: BeaconBlockHeader,
    #[superstruct(only(Capella, Deneb))]
    pub execution: ExecutionPayloadHeader,
    #[superstruct(only(Capella, Deneb))]
    pub execution_branch: FixedVector<B256, typenum::U4>,
}

impl Default for LightClientHeader {
    fn default() -> Self {
        LightClientHeader::Bellatrix(LightClientHeaderBellatrix::default())
    }
}

#[derive(Debug, Clone, Default, Encode, TreeHash, Serialize, Deserialize, Decode, PartialEq)]
pub struct SyncCommittee<S: ConsensusSpec> {
    pub pubkeys: FixedVector<PublicKey, S::SyncCommitteeSize>,
    pub aggregate_pubkey: PublicKey,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, Encode, Decode, TreeHash)]
pub struct SyncAggregate<S: ConsensusSpec> {
    pub sync_committee_bits: BitVector<S::SyncCommitteeSize>,
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

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct GenericUpdate<S: ConsensusSpec> {
    pub attested_header: LightClientHeader,
    pub sync_aggregate: SyncAggregate<S>,
    pub signature_slot: u64,
    pub next_sync_committee: Option<SyncCommittee<S>>,
    pub next_sync_committee_branch: Option<FixedVector<B256, typenum::U5>>,
    pub finalized_header: Option<LightClientHeader>,
    pub finality_branch: Option<FixedVector<B256, typenum::U6>>,
}

impl<S: ConsensusSpec> From<&Update<S>> for GenericUpdate<S> {
    fn from(update: &Update<S>) -> Self {
        Self {
            attested_header: update.attested_header.clone(),
            sync_aggregate: update.sync_aggregate.clone(),
            signature_slot: update.signature_slot,
            next_sync_committee: default_to_none(update.next_sync_committee.clone()),
            next_sync_committee_branch: default_to_none(update.next_sync_committee_branch.clone()),
            finalized_header: default_header_to_none(update.finalized_header.clone()),
            finality_branch: default_to_none(update.finality_branch.clone()),
        }
    }
}

impl<S: ConsensusSpec> From<&FinalityUpdate<S>> for GenericUpdate<S> {
    fn from(update: &FinalityUpdate<S>) -> Self {
        Self {
            attested_header: update.attested_header.clone(),
            sync_aggregate: update.sync_aggregate.clone(),
            signature_slot: update.signature_slot,
            next_sync_committee: None,
            next_sync_committee_branch: None,
            finalized_header: default_header_to_none(update.finalized_header.clone()),
            finality_branch: default_to_none(update.finality_branch.clone()),
        }
    }
}

impl<S: ConsensusSpec> From<&OptimisticUpdate<S>> for GenericUpdate<S> {
    fn from(update: &OptimisticUpdate<S>) -> Self {
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

fn default_to_none<T: Default + PartialEq>(value: T) -> Option<T> {
    if value == T::default() {
        None
    } else {
        Some(value)
    }
}

fn default_header_to_none(value: LightClientHeader) -> Option<LightClientHeader> {
    match &value {
        LightClientHeader::Bellatrix(header) => {
            if header.beacon == BeaconBlockHeader::default() {
                None
            } else {
                Some(value)
            }
        }
        LightClientHeader::Capella(header) => match &header.execution {
            ExecutionPayloadHeader::Bellatrix(payload_header) => {
                let is_default = header.beacon == BeaconBlockHeader::default()
                    && payload_header == &ExecutionPayloadHeaderBellatrix::default();

                if is_default {
                    None
                } else {
                    Some(value)
                }
            }
            ExecutionPayloadHeader::Capella(payload_header) => {
                let is_default = header.beacon == BeaconBlockHeader::default()
                    && payload_header == &ExecutionPayloadHeaderCapella::default();

                if is_default {
                    None
                } else {
                    Some(value)
                }
            }
            ExecutionPayloadHeader::Deneb(payload_header) => {
                let is_default = header.beacon == BeaconBlockHeader::default()
                    && payload_header == &ExecutionPayloadHeaderDeneb::default();

                if is_default {
                    None
                } else {
                    Some(value)
                }
            }
        },
        LightClientHeader::Deneb(header) => match &header.execution {
            ExecutionPayloadHeader::Bellatrix(payload_header) => {
                let is_default = header.beacon == BeaconBlockHeader::default()
                    && payload_header == &ExecutionPayloadHeaderBellatrix::default();

                if is_default {
                    None
                } else {
                    Some(value)
                }
            }
            ExecutionPayloadHeader::Capella(payload_header) => {
                let is_default = header.beacon == BeaconBlockHeader::default()
                    && payload_header == &ExecutionPayloadHeaderCapella::default();

                if is_default {
                    None
                } else {
                    Some(value)
                }
            }
            ExecutionPayloadHeader::Deneb(payload_header) => {
                let is_default = header.beacon == BeaconBlockHeader::default()
                    && payload_header == &ExecutionPayloadHeaderDeneb::default();

                if is_default {
                    None
                } else {
                    Some(value)
                }
            }
        },
    }
}
