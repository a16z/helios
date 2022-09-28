use thiserror::Error;

#[derive(Debug, Error)]
pub enum VerifyUpdateError {
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
    InvalidSyncCommitteeProof,
    #[error("invalid sync committee signature")]
    InvalidSignature,
}

#[derive(Debug, Error)]
pub enum BootstrapError {
    #[error("invalid sync committee proof")]
    InvalidSyncCommitteeProof,
    #[error("invalid header hash")]
    InvalidHash,
}
