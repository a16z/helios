#[cfg(not(target_arch = "wasm32"))]
pub use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(not(target_arch = "wasm32"))]
pub use tokio::time::{interval, interval_at, Instant};
#[cfg(target_arch = "wasm32")]
pub use wasmtimer::{
    std::{Instant, SystemTime, UNIX_EPOCH},
    tokio::{interval, interval_at},
};
