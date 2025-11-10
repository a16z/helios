use std::collections::HashMap;
use std::sync::Arc;

use alloy::primitives::U256;
use alloy::rpc::types::Filter;
use tokio::sync::RwLock;

#[derive(Default)]
pub struct FilterState {
    filters: Arc<RwLock<HashMap<U256, FilterType>>>,
}

#[derive(Clone)]
pub enum FilterType {
    Blocks { start_block: u64, last_poll: u64 },
    Logs { filter: Box<Filter>, last_poll: u64 },
}

impl FilterState {
    pub async fn new_filter(&self, filter: Filter) -> U256 {
        let id = U256::random();
        let filter_type = FilterType::Logs {
            filter: Box::new(filter),
            last_poll: 0,
        };

        self.filters.write().await.insert(id, filter_type);
        id
    }

    pub async fn new_block_filter(&self, start_block: u64) -> U256 {
        let id = U256::random();
        let filter_type = FilterType::Blocks {
            start_block,
            last_poll: 0,
        };

        self.filters.write().await.insert(id, filter_type);
        id
    }

    pub async fn uninstall_filter(&self, id: U256) -> bool {
        self.filters.write().await.remove(&id).is_some()
    }

    pub async fn get_filter(&self, id: U256) -> Option<FilterType> {
        self.filters.read().await.get(&id).cloned()
    }

    pub async fn update_last_poll(&self, id: U256, last_poll: u64) {
        if let Some(filter_type) = self.filters.write().await.get_mut(&id) {
            match filter_type {
                FilterType::Logs {
                    last_poll: ref mut lp,
                    ..
                } => *lp = last_poll,
                FilterType::Blocks {
                    last_poll: ref mut lp,
                    ..
                } => *lp = last_poll,
            }
        }
    }
}
