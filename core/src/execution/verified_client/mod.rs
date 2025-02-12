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

pub mod api;
pub mod rpc;

#[async_trait]
pub trait VerifiableMethods<N: NetworkSpec, R: ExecutionRpc<N>> {
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
}

#[derive(Clone)]
pub enum VerifiableMethodsClient<N: NetworkSpec, R: ExecutionRpc<N>, A: VerifiableApi<N>> {
    Api(api::VerifiableMethodsApi<N, R, A>),
    Rpc(rpc::VerifiableMethodsRpc<N, R>),
}

impl<N: NetworkSpec, R: ExecutionRpc<N>, A: VerifiableApi<N>> VerifiableMethodsClient<N, R, A> {
    // Manual dispatch
    pub fn client(&self) -> &dyn VerifiableMethods<N, R> {
        match self {
            VerifiableMethodsClient::Api(client) => client,
            VerifiableMethodsClient::Rpc(client) => client,
        }
    }
}
