use std::sync::Arc;

use async_trait::async_trait;
use eyre::Result;
use tracing::info;

use helios_common::{
    execution_mode::ExecutionMode, execution_spec::ExecutionSpec, network_spec::NetworkSpec,
};
use helios_verifiable_api_client::VerifiableApi;

use super::rpc::ExecutionRpc;
use super::state::State;

pub mod rpc;
pub mod verifiable_api;

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait ExecutionInner<N: NetworkSpec>: ExecutionSpec<N> {
    fn new(url: &str, state: State<N>) -> Result<Self>
    where
        Self: Sized;
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
