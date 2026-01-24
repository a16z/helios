use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
};

use alloy::{
    eips::BlockId,
    primitives::{Address, Bytes, B256},
    rpc::types::{Filter, Log},
};
use async_trait::async_trait;
use eyre::Result;

use crate::state_cache::{NoopStateCache, StateCache};
use crate::{network_spec::NetworkSpec, types::Account};

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait ExecutionProvider<N: NetworkSpec>:
    AccountProvider<N>
    + BlockProvider<N>
    + TransactionProvider<N>
    + ReceiptProvider<N>
    + LogProvider<N>
    + ExecutionHintProvider<N>
    + Send
    + Sync
    + 'static
{
    fn state_cache(&self) -> Arc<dyn StateCache> {
        static NOOP: OnceLock<Arc<NoopStateCache>> = OnceLock::new();
        NOOP.get_or_init(|| Arc::new(NoopStateCache)).clone()
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait AccountProvider<N: NetworkSpec> {
    async fn get_account(
        &self,
        address: Address,
        slots: &[B256],
        with_code: bool,
        block_id: BlockId,
    ) -> Result<Account>;

    /// Get code for an address, verified against a known code_hash.
    /// The code_hash MUST come from a verified account proof.
    async fn get_verified_code(&self, address: Address, code_hash: B256) -> Result<Bytes>;
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait BlockProvider<N: NetworkSpec>: Send + Sync + 'static {
    async fn push_block(&self, block: N::BlockResponse, block_id: BlockId);
    async fn get_block(&self, block_id: BlockId, full_tx: bool)
        -> Result<Option<N::BlockResponse>>;
    async fn get_untrusted_block(
        &self,
        block_id: BlockId,
        full_tx: bool,
    ) -> Result<Option<N::BlockResponse>>;
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait TransactionProvider<N: NetworkSpec> {
    async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<B256>;
    async fn get_transaction(&self, hash: B256) -> Result<Option<N::TransactionResponse>>;
    async fn get_transaction_by_location(
        &self,
        block_id: BlockId,
        index: u64,
    ) -> Result<Option<N::TransactionResponse>>;
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait ReceiptProvider<N: NetworkSpec> {
    async fn get_receipt(&self, hash: B256) -> Result<Option<N::ReceiptResponse>>;
    async fn get_block_receipts(&self, block: BlockId) -> Result<Option<Vec<N::ReceiptResponse>>>;
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait LogProvider<N: NetworkSpec> {
    async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>>;
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait ExecutionHintProvider<N: NetworkSpec> {
    async fn get_execution_hint(
        &self,
        call: &N::TransactionRequest,
        validate: bool,
        block_id: BlockId,
    ) -> Result<HashMap<Address, Account>>;
}
