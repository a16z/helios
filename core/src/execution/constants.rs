pub const PARALLEL_QUERY_BATCH_SIZE: usize = 20;

// We currently limit the max number of logs to fetch,
// to avoid blocking the client for too long.
// Default value is 100 logs, but can be configured via --max-logs flag
pub const DEFAULT_MAX_SUPPORTED_LOGS: usize = 100;

pub const MAX_STATE_HISTORY_LENGTH: usize = 64;
