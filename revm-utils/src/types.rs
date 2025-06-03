use revm::context::DBErrorMarker;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("state missing")]
    StateMissing,
    #[error("should never be called")]
    Unimplemented,
}

impl DBErrorMarker for DatabaseError {}
