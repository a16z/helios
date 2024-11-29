pub const PARALLEL_QUERY_BATCH_SIZE: usize = 20;

// We currently limit the max number of logs to fetch,
// to avoid blocking the client for too long.
pub const MAX_SUPPORTED_LOGS_NUMBER: usize = 5;
pub const BLOB_BASE_FEE_UPDATE_FRACTION: u64 = 3338477;
pub const MIN_BASE_FEE_PER_BLOB_GAS: u64 = 1;
