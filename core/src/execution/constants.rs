pub const PARALLEL_QUERY_BATCH_SIZE: usize = 20;

// We currently limit the max number of logs to fetch,
// to avoid blocking the client for too long.
pub const MAX_SUPPORTED_LOGS_NUMBER: usize = 5;

pub const MAX_STATE_HISTORY_LENGTH: usize = 64;
