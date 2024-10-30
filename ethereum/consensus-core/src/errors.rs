use alloy::primitives::B256;
use thiserror::Error;

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
    #[error("invalid execution payload proof")]
    InvalidExecutionPayloadProof,
    #[error("invalid sync committee signature")]
    InvalidSignature,
    #[error("invalid header hash found: {0}, expected: {1}")]
    InvalidHeaderHash(B256, B256),
    #[error("payload not found for slot: {0}")]
    PayloadNotFound(u64),
    #[error("checkpoint is too old")]
    CheckpointTooOld,
    #[error("consensus rpc is for the incorrect network")]
    IncorrectRpcNetwork,
    #[error("finalized header invalid or absent")]
    InvalidFinalizedHeader,
}
