use std::marker::PhantomData;

use alloy::primitives::{Address, FixedBytes, B256, U256};
use alloy_rlp::RlpEncodable;
use eyre::Result;
use serde::{Deserialize, Serialize};
use ssz_derive::{Decode, Encode};
use ssz_types::{serde_utils::quoted_u64_var_list, BitList, BitVector, FixedVector, VariableList};
use superstruct::superstruct;
use tree_hash_derive::TreeHash;
use typenum::Unsigned;

use crate::consensus_spec::ConsensusSpec;

use self::{
    bls::{PublicKey, Signature},
    bytes::{ByteList, ByteVector},
    transaction::Transaction,
};

pub mod bls;
pub mod bytes;
mod serde_utils;
pub mod transaction;

pub type LogsBloom = ByteVector<typenum::U256>;
pub type KZGCommitment = ByteVector<typenum::U48>;

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
    variants(Bellatrix, Capella, Deneb, Electra),
    variant_attributes(
        derive(Deserialize, Clone, Debug, Encode, TreeHash, Default),
        serde(deny_unknown_fields),
        serde(bound = "S: ConsensusSpec"),
    ),
    specific_variant_attributes(Electra(tree_hash(
        struct_behaviour = "profile",
        max_fields = "typenum::U64"
    )))
)]
#[derive(Encode, TreeHash, Deserialize, Debug, Clone)]
#[serde(untagged)]
#[serde(bound = "S: ConsensusSpec")]
#[ssz(enum_behaviour = "transparent")]
#[tree_hash(enum_behaviour = "transparent_stable")]
pub struct BeaconBlockBody<S: ConsensusSpec> {
    randao_reveal: Signature,
    eth1_data: Eth1Data,
    graffiti: B256,
    proposer_slashings: VariableList<ProposerSlashing, S::MaxProposerSlashings>,

    #[superstruct(
        only(Bellatrix, Capella, Deneb),
        partial_getter(rename = "attester_slashings_base")
    )]
    attester_slashings: VariableList<AttesterSlashing<S>, S::MaxAttesterSlashings>,
    #[superstruct(only(Electra), partial_getter(rename = "attester_slashings_electra"))]
    attester_slashings: VariableList<AttesterSlashing<S>, S::MaxAttesterSlashingsElectra>,

    #[superstruct(
        only(Bellatrix, Capella, Deneb),
        partial_getter(rename = "attestations_base")
    )]
    attestations: VariableList<Attestation<S>, S::MaxAttestations>,
    #[superstruct(only(Electra), partial_getter(rename = "attestations_electra"))]
    attestations: VariableList<Attestation<S>, S::MaxAttestationsElectra>,

    deposits: VariableList<Deposit, S::MaxDeposits>,
    voluntary_exits: VariableList<SignedVoluntaryExit, S::MaxVoluntaryExits>,
    sync_aggregate: SyncAggregate<S>,
    pub execution_payload: ExecutionPayload<S>,
    #[superstruct(only(Capella, Deneb, Electra))]
    bls_to_execution_changes: VariableList<SignedBlsToExecutionChange, S::MaxBlsToExecutionChanged>,
    #[superstruct(only(Deneb, Electra))]
    blob_kzg_commitments: VariableList<KZGCommitment, S::MaxBlobKzgCommitments>,
    #[superstruct(only(Electra))]
    execution_requests: ExecutionRequests<S>,
}

impl<S: ConsensusSpec> Default for BeaconBlockBody<S> {
    fn default() -> Self {
        BeaconBlockBody::Electra(BeaconBlockBodyElectra::default())
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
    variants(Bellatrix, Capella, Deneb, Electra),
    variant_attributes(
        derive(Default, Debug, Deserialize, Encode, TreeHash, Clone),
        serde(deny_unknown_fields),
        serde(bound = "S: ConsensusSpec"),
    ),
    specific_variant_attributes(Electra(tree_hash(
        struct_behaviour = "profile",
        max_fields = "typenum::U64"
    )))
)]
#[derive(Debug, Deserialize, Clone, Encode, TreeHash)]
#[serde(untagged)]
#[serde(bound = "S: ConsensusSpec")]
#[ssz(enum_behaviour = "transparent")]
#[tree_hash(enum_behaviour = "transparent_stable")]
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
    #[superstruct(only(Capella, Deneb, Electra))]
    pub withdrawals: VariableList<Withdrawal, S::MaxWithdrawals>,
    #[superstruct(only(Deneb, Electra))]
    #[serde(with = "serde_utils::u64")]
    pub blob_gas_used: u64,
    #[superstruct(only(Deneb, Electra))]
    #[serde(with = "serde_utils::u64")]
    pub excess_blob_gas: u64,
    #[superstruct(only(Electra))]
    pub system_logs_root: B256,
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
    variants(Bellatrix, Capella, Deneb, Electra),
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
    ),
    specific_variant_attributes(Electra(tree_hash(
        struct_behaviour = "profile",
        max_fields = "typenum::U64"
    )))
)]
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode, TreeHash, PartialEq)]
#[serde(untagged)]
#[ssz(enum_behaviour = "transparent")]
#[tree_hash(enum_behaviour = "transparent_stable")]
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
    #[superstruct(only(Capella, Deneb, Electra))]
    pub withdrawals_root: B256,
    #[superstruct(only(Deneb, Electra))]
    #[serde(with = "serde_utils::u64")]
    pub blob_gas_used: u64,
    #[superstruct(only(Deneb, Electra))]
    #[serde(with = "serde_utils::u64")]
    pub excess_blob_gas: u64,
    #[superstruct(only(Electra))]
    pub system_logs_root: B256,
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

#[superstruct(
    variants(Base, Electra),
    variant_attributes(
        derive(Deserialize, Debug, Default, Encode, TreeHash, Clone,),
        serde(deny_unknown_fields),
    ),
    specific_variant_attributes(Electra(tree_hash(
        struct_behaviour = "profile",
        max_fields = "typenum::U8"
    )))
)]
#[derive(Deserialize, Debug, Encode, TreeHash, Clone)]
#[serde(bound = "S: ConsensusSpec")]
#[serde(untagged)]
#[ssz(enum_behaviour = "transparent")]
#[tree_hash(enum_behaviour = "transparent_stable")]
struct IndexedAttestation<S: ConsensusSpec> {
    #[serde(with = "quoted_u64_var_list")]
    #[superstruct(only(Base), partial_getter(rename = "attesting_indices_base"))]
    attesting_indices: VariableList<u64, S::MaxValidatorsPerCommitee>,
    #[serde(with = "quoted_u64_var_list")]
    #[superstruct(only(Electra), partial_getter(rename = "attesting_indices_electra"))]
    attesting_indices: VariableList<u64, S::MaxValidatorsPerSlot>,
    data: AttestationData,
    signature: Signature,
}

impl<S: ConsensusSpec> Default for IndexedAttestation<S> {
    fn default() -> Self {
        IndexedAttestation::Electra(IndexedAttestationElectra::default())
    }
}

#[superstruct(
    variants(Base, Electra),
    variant_attributes(
        derive(Deserialize, Debug, Encode, TreeHash, Clone,),
        serde(deny_unknown_fields),
    ),
    specific_variant_attributes(Electra(tree_hash(
        struct_behaviour = "profile",
        max_fields = "typenum::U8"
    ))),
    //ref_attributes(derive(TreeHash), tree_hash(enum_behaviour = "transparent")),
)]
#[derive(Deserialize, Debug, Encode, TreeHash, Clone)]
#[serde(bound = "S: ConsensusSpec")]
#[serde(untagged)]
#[ssz(enum_behaviour = "transparent")]
#[tree_hash(enum_behaviour = "transparent_stable")]
pub struct Attestation<S: ConsensusSpec> {
    #[superstruct(only(Base), partial_getter(rename = "aggregation_bits_base"))]
    aggregation_bits: BitList<S::MaxValidatorsPerCommitee>,
    #[superstruct(only(Electra), partial_getter(rename = "aggregation_bits_electra"))]
    aggregation_bits: BitList<S::MaxValidatorsPerSlot>,
    data: AttestationData,
    signature: Signature,
    #[superstruct(only(Electra))]
    committee_bits: BitVector<S::MaxCommitteesPerSlot>,
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

#[derive(Deserialize, Debug, Default, Encode, TreeHash, Clone)]
#[tree_hash(struct_behaviour = "profile", max_fields = "typenum::U16")]
pub struct ExecutionRequests<S: ConsensusSpec> {
    deposits: VariableList<DepositRequest, S::MaxDepositRequests>,
    withdrawals: VariableList<WithdrawalRequest, S::MaxWithdrawalRequests>,
    consolidations: VariableList<ConsolidationRequest, S::MaxConsolidationRequests>,
}

#[derive(Deserialize, Debug, Default, Encode, TreeHash, Clone)]
pub struct DepositRequest {
    pubkey: PublicKey,
    withdrawal_credentials: B256,
    #[serde(with = "serde_utils::u64")]
    amount: u64,
    signature: Signature,
    #[serde(with = "serde_utils::u64")]
    slot: u64,
}

#[derive(Deserialize, Debug, Default, Encode, TreeHash, Clone)]
pub struct WithdrawalRequest {
    source_address: Address,
    validator_pubkey: PublicKey,
    #[serde(with = "serde_utils::u64")]
    amount: u64,
}

#[derive(Deserialize, Debug, Default, Encode, TreeHash, Clone)]
pub struct ConsolidationRequest {
    source_address: Address,
    source_pubkey: PublicKey,
    target_pubkey: PublicKey,
}

#[superstruct(
    variants(Base, Electra),
    variant_attributes(
        derive(Deserialize, Debug, Decode),
        serde(deny_unknown_fields),
        serde(bound = "S: ConsensusSpec"),
    )
)]
#[derive(Deserialize, Debug, Decode)]
#[serde(untagged)]
#[serde(bound = "S: ConsensusSpec")]
#[ssz(enum_behaviour = "transparent")]
pub struct Bootstrap<S: ConsensusSpec> {
    pub header: LightClientHeader,
    pub current_sync_committee: SyncCommittee<S>,
    #[superstruct(
        only(Base),
        partial_getter(rename = "current_sync_committee_branch_base")
    )]
    pub current_sync_committee_branch: FixedVector<B256, typenum::U5>,
    #[superstruct(
        only(Electra),
        partial_getter(rename = "current_sync_committee_branch_electra")
    )]
    pub current_sync_committee_branch: FixedVector<B256, typenum::U8>,
}

impl<S: ConsensusSpec> Bootstrap<S> {
    pub fn current_sync_committee_branch(&self) -> &[B256] {
        match self {
            Bootstrap::Base(inner) => &inner.current_sync_committee_branch,
            Bootstrap::Electra(inner) => &inner.current_sync_committee_branch,
        }
    }
}

#[superstruct(
    variants(Base, Electra),
    variant_attributes(
        derive(Serialize, Deserialize, Debug, Clone, Decode,),
        serde(deny_unknown_fields),
        serde(bound = "S: ConsensusSpec"),
    )
)]
#[derive(Serialize, Deserialize, Debug, Clone, Decode)]
#[serde(untagged)]
#[serde(bound = "S: ConsensusSpec")]
#[ssz(enum_behaviour = "transparent")]
pub struct Update<S: ConsensusSpec> {
    pub attested_header: LightClientHeader,
    pub next_sync_committee: SyncCommittee<S>,
    #[superstruct(only(Base), partial_getter(rename = "next_sync_committee_branch_base"))]
    pub next_sync_committee_branch: FixedVector<B256, typenum::U5>,
    #[superstruct(
        only(Electra),
        partial_getter(rename = "next_sync_committee_branch_electra")
    )]
    pub next_sync_committee_branch: FixedVector<B256, typenum::U6>,
    pub finalized_header: LightClientHeader,
    #[superstruct(only(Base), partial_getter(rename = "finality_branch_base"))]
    pub finality_branch: FixedVector<B256, typenum::U6>,
    #[superstruct(only(Electra), partial_getter(rename = "finality_branch_electra"))]
    pub finality_branch: FixedVector<B256, typenum::U7>,
    pub sync_aggregate: SyncAggregate<S>,
    #[serde(with = "serde_utils::u64")]
    pub signature_slot: u64,
}

impl<S: ConsensusSpec> Update<S> {
    pub fn next_sync_committee_branch(&self) -> &[B256] {
        match self {
            Update::Base(inner) => &inner.next_sync_committee_branch,
            Update::Electra(inner) => &inner.next_sync_committee_branch,
        }
    }

    pub fn finality_branch(&self) -> &[B256] {
        match self {
            Update::Base(inner) => &inner.finality_branch,
            Update::Electra(inner) => &inner.finality_branch,
        }
    }
}

#[superstruct(
    variants(Base, Electra),
    variant_attributes(
        derive(Serialize, Deserialize, Debug, Clone, Decode,),
        serde(deny_unknown_fields),
        serde(bound = "S: ConsensusSpec"),
    )
)]
#[derive(Serialize, Deserialize, Debug, Clone, Decode)]
#[serde(untagged)]
#[serde(bound = "S: ConsensusSpec")]
#[ssz(enum_behaviour = "transparent")]
pub struct FinalityUpdate<S: ConsensusSpec> {
    pub attested_header: LightClientHeader,
    pub finalized_header: LightClientHeader,
    #[superstruct(only(Base), partial_getter(rename = "finality_branch_base"))]
    pub finality_branch: FixedVector<B256, typenum::U6>,
    #[superstruct(only(Electra), partial_getter(rename = "finality_branch_electra"))]
    pub finality_branch: FixedVector<B256, typenum::U7>,
    pub sync_aggregate: SyncAggregate<S>,
    #[serde(with = "serde_utils::u64")]
    pub signature_slot: u64,
}

impl<S: ConsensusSpec> FinalityUpdate<S> {
    pub fn finality_branch(&self) -> &[B256] {
        match self {
            FinalityUpdate::Base(inner) => &inner.finality_branch,
            FinalityUpdate::Electra(inner) => &inner.finality_branch,
        }
    }
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
    variants(Bellatrix, Capella, Deneb, Electra),
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
    #[superstruct(only(Capella, Deneb, Electra))]
    pub execution: ExecutionPayloadHeader,
    #[superstruct(only(Capella, Deneb, Electra))]
    pub execution_branch: FixedVector<B256, typenum::U7>,
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
    pub electra: Fork,
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
    pub next_sync_committee_branch: Option<Vec<B256>>,
    pub finalized_header: Option<LightClientHeader>,
    pub finality_branch: Option<Vec<B256>>,
}

impl<S: ConsensusSpec> From<&Update<S>> for GenericUpdate<S> {
    fn from(update: &Update<S>) -> Self {
        Self {
            attested_header: update.attested_header().clone(),
            sync_aggregate: update.sync_aggregate().clone(),
            signature_slot: *update.signature_slot(),
            next_sync_committee: default_to_none(update.next_sync_committee().clone()),
            next_sync_committee_branch: default_branch_to_none(update.next_sync_committee_branch()),
            finalized_header: default_header_to_none(update.finalized_header().clone()),
            finality_branch: default_branch_to_none(update.finality_branch()),
        }
    }
}

impl<S: ConsensusSpec> From<&FinalityUpdate<S>> for GenericUpdate<S> {
    fn from(update: &FinalityUpdate<S>) -> Self {
        Self {
            attested_header: update.attested_header().clone(),
            sync_aggregate: update.sync_aggregate().clone(),
            signature_slot: *update.signature_slot(),
            next_sync_committee: None,
            next_sync_committee_branch: None,
            finalized_header: default_header_to_none(update.finalized_header().clone()),
            finality_branch: default_branch_to_none(update.finality_branch()),
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

fn default_branch_to_none(value: &[B256]) -> Option<Vec<B256>> {
    for elem in value {
        if !elem.is_zero() {
            return Some(value.to_vec());
        }
    }

    None
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
            ExecutionPayloadHeader::Electra(payload_header) => {
                let is_default = header.beacon == BeaconBlockHeader::default()
                    && payload_header == &ExecutionPayloadHeaderElectra::default();

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
            ExecutionPayloadHeader::Electra(payload_header) => {
                let is_default = header.beacon == BeaconBlockHeader::default()
                    && payload_header == &ExecutionPayloadHeaderElectra::default();

                if is_default {
                    None
                } else {
                    Some(value)
                }
            }
        },
        LightClientHeader::Electra(header) => match &header.execution {
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
            ExecutionPayloadHeader::Electra(payload_header) => {
                let is_default = header.beacon == BeaconBlockHeader::default()
                    && payload_header == &ExecutionPayloadHeaderElectra::default();

                if is_default {
                    None
                } else {
                    Some(value)
                }
            }
        },
    }
}
