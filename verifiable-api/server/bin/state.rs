use std::marker::PhantomData;
use std::sync::Arc;

use eyre::Result;

use helios_common::network_spec::NetworkSpec;
use helios_core::execution::rpc::ExecutionRpc;

pub struct ExecutionClient<N: NetworkSpec, R: ExecutionRpc<N>> {
    pub rpc: R,
    _marker: PhantomData<N>,
}

impl<N: NetworkSpec, R: ExecutionRpc<N>> ExecutionClient<N, R> {
    pub fn new(rpc: &str) -> Result<Self> {
        Ok(ExecutionClient::<N, R> {
            rpc: ExecutionRpc::new(rpc)?,
            _marker: PhantomData::default(),
        })
    }
}

#[derive(Clone)]
pub struct ApiState<N: NetworkSpec, R: ExecutionRpc<N>> {
    pub execution_client: Arc<ExecutionClient<N, R>>,
}
