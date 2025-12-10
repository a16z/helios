use revm::context::DBErrorMarker;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("state missing")]
    StateMissing,
    #[error("invalid state override: {0}")]
    InvalidStateOverride(String),
    #[error("should never be called")]
    Unimplemented,
}

impl DBErrorMarker for DatabaseError {}
