pub const PARALLEL_QUERY_BATCH_SIZE: usize = 20;

// We currently limit the max number of blocks to prove a list of logs,
// to avoid blocking the client for too long.
pub const MAX_SUPPORTED_BLOCKS_TO_PROVE_FOR_LOGS: usize = 5;

pub const MAX_STATE_HISTORY_LENGTH: usize = 64;
