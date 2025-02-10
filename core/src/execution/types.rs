use std::collections::HashMap;

use alloy::primitives::{B256, U256};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub balance: U256,
    pub nonce: u64,
    pub code_hash: B256,
    pub code: Vec<u8>,
    pub storage_hash: B256,
    pub slots: HashMap<B256, U256>,
}
