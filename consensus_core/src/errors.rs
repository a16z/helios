use ethers_core::types::H256;
use thiserror::Error;

use common::types::BlockTag;

#[derive(Debug, Error)]
pub enum ConsensusError {
    #[error("insufficient participation")]
    InsufficientParticipation,
    #[error("invalid timestamp")]
    InvalidTimestamp,
    #[error("invalid sync committee period")]
    InvalidPeriod,
    #[error("update not relevant")]
    NotRelevant,
    #[error("invalid finality proof")]
    InvalidFinalityProof,
    #[error("invalid next sync committee proof")]
    InvalidNextSyncCommitteeProof,
    #[error("invalid current sync committee proof")]
    InvalidCurrentSyncCommitteeProof,
    #[error("invalid sync committee signature")]
    InvalidSignature,
    #[error("invalid header hash found: {0}, expected: {1}")]
    InvalidHeaderHash(String, String),
    #[error("payload not found for slot: {0}")]
    PayloadNotFound(u64),
    #[error("checkpoint is too old")]
    CheckpointTooOld,
    #[error("consensus rpc is for the incorrect network")]
    IncorrectRpcNetwork,
}

#[derive(Debug, Error)]
#[error("block not available: {block}")]
pub struct BlockNotFoundError {
    block: BlockTag,
}

impl BlockNotFoundError {
    pub fn new(block: BlockTag) -> Self {
        Self { block }
    }
}

#[derive(Debug, Error)]
#[error("slot not found: {slot:?}")]
pub struct SlotNotFoundError {
    slot: H256,
}

impl SlotNotFoundError {
    pub fn new(slot: H256) -> Self {
        Self { slot }
    }
}

#[derive(Debug, Error)]
#[error("rpc error on method: {method}, message: {error}")]
pub struct RpcError<E: ToString> {
    method: String,
    error: E,
}

impl<E: ToString> RpcError<E> {
    pub fn new(method: &str, err: E) -> Self {
        Self {
            method: method.to_string(),
            error: err,
        }
    }
}
