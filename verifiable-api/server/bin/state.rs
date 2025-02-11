use std::marker::PhantomData;
use std::sync::Arc;

use eyre::Result;

use helios_common::network_spec::NetworkSpec;
use helios_core::execution::rpc::ExecutionRpc;

#[derive(Clone)]
pub struct ApiState<N: NetworkSpec, R: ExecutionRpc<N>> {
    pub rpc: Arc<R>,
    _marker: PhantomData<N>,
}

impl<N: NetworkSpec, R: ExecutionRpc<N>> ApiState<N, R> {
    pub fn new(rpc: &str) -> Result<Self> {
        Ok(Self {
            rpc: Arc::new(ExecutionRpc::new(rpc)?),
            _marker: PhantomData::default(),
        })
    }
}
