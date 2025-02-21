use std::collections::HashMap;

use alloy::eips::BlockId;
use alloy::primitives::{Address, B256, U256};
use alloy::rpc::types::{Filter, FilterChanges, Log};
use async_trait::async_trait;
use eyre::Result;

use helios_common::{
    network_spec::NetworkSpec,
    types::{Account, BlockTag},
};
use helios_verifiable_api_client::VerifiableApi;

use super::rpc::ExecutionRpc;
use super::state::State;

pub mod rpc;
pub mod verifiable_api;

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait ExecutionMethods<N: NetworkSpec, R: ExecutionRpc<N>>: Send + Sync {
    fn new(url: &str, state: State<N, R>) -> Result<Self>
    where
        Self: Sized;
    async fn get_account(
        &self,
        address: Address,
        slots: Option<&[B256]>,
        tag: BlockTag,
    ) -> Result<Account>;
    async fn get_transaction_receipt(&self, tx_hash: B256) -> Result<Option<N::ReceiptResponse>>;
    async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>>;
    async fn get_filter_changes(&self, filter_id: U256) -> Result<FilterChanges>;
    async fn get_filter_logs(&self, filter_id: U256) -> Result<Vec<Log>>;
    async fn create_access_list(
        &self,
        tx: &N::TransactionRequest,
        block: Option<BlockId>,
    ) -> Result<HashMap<Address, Account>>;
    async fn chain_id(&self) -> Result<u64>;
    async fn get_block_receipts(&self, block: BlockId) -> Result<Option<Vec<N::ReceiptResponse>>>;
    async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<B256>;
    async fn new_filter(&self, filter: &Filter) -> Result<U256>;
    async fn new_block_filter(&self) -> Result<U256>;
    async fn new_pending_transaction_filter(&self) -> Result<U256>;
    async fn uninstall_filter(&self, filter_id: U256) -> Result<bool>;
}

#[derive(Clone)]
pub enum ExecutionMethodsClient<N: NetworkSpec, R: ExecutionRpc<N>, A: VerifiableApi<N>> {
    Api(verifiable_api::ExecutionVerifiableApiClient<N, R, A>),
    Rpc(rpc::ExecutionRpcClient<N, R>),
}

impl<N: NetworkSpec, R: ExecutionRpc<N>, A: VerifiableApi<N>> ExecutionMethodsClient<N, R, A> {
    // manual dynamic dispatch
    pub fn client(&self) -> &dyn ExecutionMethods<N, R> {
        match self {
            ExecutionMethodsClient::Api(client) => client,
            ExecutionMethodsClient::Rpc(client) => client,
        }
    }
}
