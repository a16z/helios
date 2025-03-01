pub const PARALLEL_QUERY_BATCH_SIZE: usize = 20;

// We currently limit the max number of logs to fetch,
// to avoid blocking the client for too long.
// Increased from 5 to 100 to better support dApps that generate more logs
// while still maintaining reasonable performance.
pub const MAX_SUPPORTED_LOGS_NUMBER: usize = 100;

pub const MAX_STATE_HISTORY_LENGTH: usize = 64;
