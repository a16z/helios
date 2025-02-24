use std::collections::HashMap;
use std::sync::Arc;

use alloy::eips::BlockId;
use alloy::primitives::{Address, B256, U256};
use alloy::rpc::types::{Filter, FilterChanges, Log};
use async_trait::async_trait;
use eyre::Result;
use tracing::info;

use helios_common::{
    execution_mode::ExecutionMode,
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
pub trait ExecutionInner<N: NetworkSpec>: Send + Sync + 'static {
    fn new(url: &str, state: State<N>) -> Result<Self>
    where
        Self: Sized;
    async fn get_account(
        &self,
        address: Address,
        slots: Option<&[B256]>,
        tag: BlockTag,
        include_code: bool,
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
    async fn get_block(&self, block_id: BlockId) -> Result<Option<N::BlockResponse>>;
    async fn get_block_receipts(&self, block: BlockId) -> Result<Option<Vec<N::ReceiptResponse>>>;
    async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<B256>;
    async fn new_filter(&self, filter: &Filter) -> Result<U256>;
    async fn new_block_filter(&self) -> Result<U256>;
    async fn new_pending_transaction_filter(&self) -> Result<U256>;
    async fn uninstall_filter(&self, filter_id: U256) -> Result<bool>;
}

#[derive(Clone)]
pub enum ExecutionInnerClient<N: NetworkSpec, R: ExecutionRpc<N>, A: VerifiableApi<N>> {
    Api(verifiable_api::ExecutionInnerVerifiableApiClient<N, A>),
    Rpc(rpc::ExecutionInnerRpcClient<N, R>),
}

impl<N: NetworkSpec, R: ExecutionRpc<N>, A: VerifiableApi<N>> ExecutionInnerClient<N, R, A> {
    /// Dynamically construct and returns one of `ExecutionInnerRpcClient` or `ExecutionInnerVerifiableApiClient`
    /// based on the given `ExecutionMode`.
    pub fn make_inner_client(
        execution_mode: ExecutionMode,
        state: State<N>,
    ) -> Result<Arc<dyn ExecutionInner<N>>> {
        let this = match execution_mode {
            ExecutionMode::VerifiableApi(api_url) => {
                info!(target: "helios::execution", "using Verifiable-API url={}", api_url);
                Self::Api(verifiable_api::ExecutionInnerVerifiableApiClient::new(
                    &api_url, state,
                )?)
            }
            ExecutionMode::Rpc(rpc_url) => {
                info!(target: "helios::execution", "Using JSON-RPC url={}", rpc_url);
                Self::Rpc(rpc::ExecutionInnerRpcClient::new(&rpc_url, state)?)
            }
        };

        Ok(match this {
            Self::Api(client) => Arc::new(client),
            Self::Rpc(client) => Arc::new(client),
        })
    }
}
