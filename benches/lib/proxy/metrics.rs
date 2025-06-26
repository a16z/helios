use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Clone, Debug, Default)]
pub struct ProxyMetrics {
    pub bytes_downloaded: Arc<AtomicU64>,
    pub bytes_uploaded: Arc<AtomicU64>,
    pub request_count: Arc<AtomicUsize>,
}

impl ProxyMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&self) {
        self.bytes_downloaded.store(0, Ordering::SeqCst);
        self.bytes_uploaded.store(0, Ordering::SeqCst);
        self.request_count.store(0, Ordering::SeqCst);
    }

    pub fn get_stats(&self) -> (u64, u64, usize) {
        (
            self.bytes_downloaded.load(Ordering::SeqCst),
            self.bytes_uploaded.load(Ordering::SeqCst),
            self.request_count.load(Ordering::SeqCst),
        )
    }
}
