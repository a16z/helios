use alloy::eips::BlockId;
use async_trait::async_trait;
use eyre::Result;
use helios_common::network_spec::NetworkSpec;

use crate::execution::providers::{AccountProvider, BlockProvider};

pub mod eip2935;

/// Provider for fetching historical blocks that may not be available through normal block caching.
///
/// Historical blocks are NOT cached to avoid interfering with the block cache consistency algorithm.
/// Different chains may use different mechanisms for historical block retrieval.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait HistoricalBlockProvider<N: NetworkSpec>: Send + Sync + 'static {
    /// Get a historical block that may not be available through normal means.
    /// Returns None if the block is not available through this provider.
    ///
    /// Note: Results are not cached to maintain block cache consistency.
    async fn get_historical_block<E>(
        &self,
        block_id: BlockId,
        full_tx: bool,
        execution_provider: &E,
    ) -> Result<Option<N::BlockResponse>>
    where
        E: BlockProvider<N> + AccountProvider<N>;
}
