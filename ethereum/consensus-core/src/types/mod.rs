use alloy::primitives::{Address, FixedBytes, B256, U256};
use eyre::Result;
use serde::{Deserialize, Serialize};
use ssz_derive::{Decode, Encode};
use ssz_types::{BitVector, FixedVector};
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
    #[superstruct(only(Capella, Deneb, Electra))]
    pub withdrawals_root: B256,
    #[superstruct(only(Deneb, Electra))]
    #[serde(with = "serde_utils::u64")]
    pub blob_gas_used: u64,
    #[superstruct(only(Deneb, Electra))]
    #[serde(with = "serde_utils::u64")]
    pub excess_blob_gas: u64,
}

impl Default for ExecutionPayloadHeader {
    fn default() -> Self {
        ExecutionPayloadHeader::Bellatrix(ExecutionPayloadHeaderBellatrix::default())
    }
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

#[superstruct(
    variants(Base, Electra, Gloas),
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
    #[superstruct(only(Base, Electra), partial_getter(rename = "header_base"))]
    pub header: LightClientHeader,
    #[superstruct(only(Gloas))]
    #[serde(rename = "header")]
    pub header_gloas: LightClientHeaderGloas,
    pub current_sync_committee: SyncCommittee<S>,
    #[superstruct(
        only(Base),
        partial_getter(rename = "current_sync_committee_branch_base")
    )]
    pub current_sync_committee_branch: FixedVector<B256, typenum::U5>,
    #[superstruct(
        only(Electra, Gloas),
        partial_getter(rename = "current_sync_committee_branch_electra")
    )]
    pub current_sync_committee_branch: FixedVector<B256, typenum::U6>,
}

impl<S: ConsensusSpec> Bootstrap<S> {
    pub fn header(&self) -> LightClientHeader {
        match self {
            Bootstrap::Base(inner) => inner.header.clone(),
            Bootstrap::Electra(inner) => inner.header.clone(),
            Bootstrap::Gloas(inner) => LightClientHeader::Gloas(inner.header_gloas.clone()),
        }
    }

    pub fn current_sync_committee_branch(&self) -> &[B256] {
        match self {
            Bootstrap::Base(inner) => &inner.current_sync_committee_branch,
            Bootstrap::Electra(inner) => &inner.current_sync_committee_branch,
            Bootstrap::Gloas(inner) => &inner.current_sync_committee_branch,
        }
    }
}

#[superstruct(
    variants(Base, Electra, Gloas),
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
    #[superstruct(only(Base, Electra), partial_getter(rename = "attested_header_base"))]
    pub attested_header: LightClientHeader,
    #[superstruct(only(Gloas))]
    #[serde(rename = "attested_header")]
    pub attested_header_gloas: LightClientHeaderGloas,
    pub next_sync_committee: SyncCommittee<S>,
    #[superstruct(only(Base), partial_getter(rename = "next_sync_committee_branch_base"))]
    pub next_sync_committee_branch: FixedVector<B256, typenum::U5>,
    #[superstruct(
        only(Electra, Gloas),
        partial_getter(rename = "next_sync_committee_branch_electra")
    )]
    pub next_sync_committee_branch: FixedVector<B256, typenum::U6>,
    #[superstruct(only(Base, Electra), partial_getter(rename = "finalized_header_base"))]
    pub finalized_header: LightClientHeader,
    #[superstruct(only(Gloas))]
    #[serde(rename = "finalized_header")]
    pub finalized_header_gloas: LightClientHeaderGloas,
    #[superstruct(only(Base), partial_getter(rename = "finality_branch_base"))]
    pub finality_branch: FixedVector<B256, typenum::U6>,
    #[superstruct(
        only(Electra, Gloas),
        partial_getter(rename = "finality_branch_electra")
    )]
    pub finality_branch: FixedVector<B256, typenum::U7>,
    pub sync_aggregate: SyncAggregate<S>,
    #[serde(with = "serde_utils::u64")]
    pub signature_slot: u64,
}

impl<S: ConsensusSpec> Update<S> {
    pub fn attested_header(&self) -> LightClientHeader {
        match self {
            Update::Base(inner) => inner.attested_header.clone(),
            Update::Electra(inner) => inner.attested_header.clone(),
            Update::Gloas(inner) => LightClientHeader::Gloas(inner.attested_header_gloas.clone()),
        }
    }

    pub fn finalized_header(&self) -> LightClientHeader {
        match self {
            Update::Base(inner) => inner.finalized_header.clone(),
            Update::Electra(inner) => inner.finalized_header.clone(),
            Update::Gloas(inner) => LightClientHeader::Gloas(inner.finalized_header_gloas.clone()),
        }
    }

    pub fn finalized_header_mut(&mut self) -> Option<&mut LightClientHeader> {
        match self {
            Update::Base(inner) => Some(&mut inner.finalized_header),
            Update::Electra(inner) => Some(&mut inner.finalized_header),
            Update::Gloas(_) => None,
        }
    }

    pub fn next_sync_committee_branch(&self) -> &[B256] {
        match self {
            Update::Base(inner) => &inner.next_sync_committee_branch,
            Update::Electra(inner) => &inner.next_sync_committee_branch,
            Update::Gloas(inner) => &inner.next_sync_committee_branch,
        }
    }

    pub fn finality_branch(&self) -> &[B256] {
        match self {
            Update::Base(inner) => &inner.finality_branch,
            Update::Electra(inner) => &inner.finality_branch,
            Update::Gloas(inner) => &inner.finality_branch,
        }
    }
}

#[superstruct(
    variants(Base, Electra, Gloas),
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
    #[superstruct(only(Base, Electra), partial_getter(rename = "attested_header_base"))]
    pub attested_header: LightClientHeader,
    #[superstruct(only(Gloas))]
    #[serde(rename = "attested_header")]
    pub attested_header_gloas: LightClientHeaderGloas,
    #[superstruct(only(Base, Electra), partial_getter(rename = "finalized_header_base"))]
    pub finalized_header: LightClientHeader,
    #[superstruct(only(Gloas))]
    #[serde(rename = "finalized_header")]
    pub finalized_header_gloas: LightClientHeaderGloas,
    #[superstruct(only(Base), partial_getter(rename = "finality_branch_base"))]
    pub finality_branch: FixedVector<B256, typenum::U6>,
    #[superstruct(
        only(Electra, Gloas),
        partial_getter(rename = "finality_branch_electra")
    )]
    pub finality_branch: FixedVector<B256, typenum::U7>,
    pub sync_aggregate: SyncAggregate<S>,
    #[serde(with = "serde_utils::u64")]
    pub signature_slot: u64,
}

impl<S: ConsensusSpec> FinalityUpdate<S> {
    pub fn attested_header(&self) -> LightClientHeader {
        match self {
            FinalityUpdate::Base(inner) => inner.attested_header.clone(),
            FinalityUpdate::Electra(inner) => inner.attested_header.clone(),
            FinalityUpdate::Gloas(inner) => {
                LightClientHeader::Gloas(inner.attested_header_gloas.clone())
            }
        }
    }

    pub fn finalized_header(&self) -> LightClientHeader {
        match self {
            FinalityUpdate::Base(inner) => inner.finalized_header.clone(),
            FinalityUpdate::Electra(inner) => inner.finalized_header.clone(),
            FinalityUpdate::Gloas(inner) => {
                LightClientHeader::Gloas(inner.finalized_header_gloas.clone())
            }
        }
    }

    pub fn finalized_header_mut(&mut self) -> Option<&mut LightClientHeader> {
        match self {
            FinalityUpdate::Base(inner) => Some(&mut inner.finalized_header),
            FinalityUpdate::Electra(inner) => Some(&mut inner.finalized_header),
            FinalityUpdate::Gloas(_) => None,
        }
    }

    pub fn finality_branch(&self) -> &[B256] {
        match self {
            FinalityUpdate::Base(inner) => &inner.finality_branch,
            FinalityUpdate::Electra(inner) => &inner.finality_branch,
            FinalityUpdate::Gloas(inner) => &inner.finality_branch,
        }
    }
}

#[superstruct(
    variants(Base, Gloas),
    variant_attributes(
        derive(Serialize, Deserialize, Debug, Clone, Decode,),
        serde(deny_unknown_fields),
        serde(bound = "S: ConsensusSpec"),
    )
)]
#[derive(Serialize, Deserialize, Debug, Clone, Decode)]
#[serde(bound = "S: ConsensusSpec")]
#[serde(untagged)]
#[ssz(enum_behaviour = "transparent")]
pub struct OptimisticUpdate<S: ConsensusSpec> {
    #[superstruct(only(Base), partial_getter(rename = "attested_header_base"))]
    pub attested_header: LightClientHeader,
    #[superstruct(only(Gloas))]
    #[serde(rename = "attested_header")]
    pub attested_header_gloas: LightClientHeaderGloas,
    pub sync_aggregate: SyncAggregate<S>,
    #[serde(with = "serde_utils::u64")]
    pub signature_slot: u64,
}

impl<S: ConsensusSpec> OptimisticUpdate<S> {
    pub fn attested_header(&self) -> LightClientHeader {
        match self {
            OptimisticUpdate::Base(inner) => inner.attested_header.clone(),
            OptimisticUpdate::Gloas(inner) => {
                LightClientHeader::Gloas(inner.attested_header_gloas.clone())
            }
        }
    }
}

#[superstruct(
    variants(Bellatrix, Capella, Deneb, Electra, Gloas),
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
    #[superstruct(only(Gloas))]
    pub execution_block_hash: B256,
    #[superstruct(only(Capella, Deneb, Electra))]
    pub execution_branch: FixedVector<B256, typenum::U4>,
    #[superstruct(only(Gloas), partial_getter(rename = "execution_branch_gloas"))]
    pub execution_branch: FixedVector<B256, typenum::U9>,
}

impl Default for LightClientHeader {
    fn default() -> Self {
        LightClientHeader::Bellatrix(LightClientHeaderBellatrix::default())
    }
}

impl LightClientHeader {
    pub fn execution_root(&self) -> B256 {
        match self {
            LightClientHeader::Bellatrix(_) => B256::ZERO,
            LightClientHeader::Capella(header) => {
                tree_hash::TreeHash::tree_hash_root(&header.execution)
            }
            LightClientHeader::Deneb(header) => {
                tree_hash::TreeHash::tree_hash_root(&header.execution)
            }
            LightClientHeader::Electra(header) => {
                tree_hash::TreeHash::tree_hash_root(&header.execution)
            }
            LightClientHeader::Gloas(header) => header.execution_block_hash,
        }
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Forks {
    pub genesis: Fork,
    pub altair: Fork,
    pub bellatrix: Fork,
    pub capella: Fork,
    pub deneb: Fork,
    pub electra: Fork,
    pub fulu: Fork,
    #[serde(default = "inactive_fork")]
    pub gloas: Fork,
}

impl Default for Forks {
    fn default() -> Self {
        Self {
            genesis: Fork::default(),
            altair: Fork::default(),
            bellatrix: Fork::default(),
            capella: Fork::default(),
            deneb: Fork::default(),
            electra: Fork::default(),
            fulu: Fork::default(),
            gloas: inactive_fork(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Fork {
    pub epoch: u64,
    pub fork_version: FixedBytes<4>,
}

fn inactive_fork() -> Fork {
    Fork {
        epoch: u64::MAX,
        ..Fork::default()
    }
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
            attested_header: update.attested_header(),
            sync_aggregate: update.sync_aggregate().clone(),
            signature_slot: *update.signature_slot(),
            next_sync_committee: default_to_none(update.next_sync_committee().clone()),
            next_sync_committee_branch: default_branch_to_none(update.next_sync_committee_branch()),
            finalized_header: default_header_to_none(update.finalized_header()),
            finality_branch: default_branch_to_none(update.finality_branch()),
        }
    }
}

impl<S: ConsensusSpec> From<&FinalityUpdate<S>> for GenericUpdate<S> {
    fn from(update: &FinalityUpdate<S>) -> Self {
        Self {
            attested_header: update.attested_header(),
            sync_aggregate: update.sync_aggregate().clone(),
            signature_slot: *update.signature_slot(),
            next_sync_committee: None,
            next_sync_committee_branch: None,
            finalized_header: default_header_to_none(update.finalized_header()),
            finality_branch: default_branch_to_none(update.finality_branch()),
        }
    }
}

impl<S: ConsensusSpec> From<&OptimisticUpdate<S>> for GenericUpdate<S> {
    fn from(update: &OptimisticUpdate<S>) -> Self {
        Self {
            attested_header: update.attested_header(),
            sync_aggregate: update.sync_aggregate().clone(),
            signature_slot: *update.signature_slot(),
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
        LightClientHeader::Gloas(header) => {
            let is_default = header.beacon == BeaconBlockHeader::default()
                && header.execution_block_hash.is_zero()
                && default_branch_to_none(&header.execution_branch).is_none();

            if is_default {
                None
            } else {
                Some(value)
            }
        }
    }
}
