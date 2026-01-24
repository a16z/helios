use alloy::primitives::{Address, B256, U256};

use crate::types::Account;

/// Read interface for a shared state/storage cache.
///
/// This is intentionally minimal and sync-only so it can be used from REVM's `Database`
/// implementation without requiring async.
pub trait StateCache: Send + Sync + 'static {
    fn get_storage(&self, address: Address, slot: B256, block_hash: B256) -> Option<U256>;

    fn get_account(
        &self,
        address: Address,
        slots: &[B256],
        block_hash: B256,
    ) -> Option<(Account, Vec<B256>)>;
}

#[derive(Debug, Default)]
pub struct NoopStateCache;

impl StateCache for NoopStateCache {
    fn get_storage(&self, _address: Address, _slot: B256, _block_hash: B256) -> Option<U256> {
        None
    }

    fn get_account(
        &self,
        _address: Address,
        _slots: &[B256],
        _block_hash: B256,
    ) -> Option<(Account, Vec<B256>)> {
        None
    }
}
