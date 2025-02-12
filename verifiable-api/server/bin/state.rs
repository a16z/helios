use std::marker::PhantomData;
use std::sync::Arc;

use eyre::Result;

use helios_common::network_spec::NetworkSpec;
use helios_core::execution::rpc::ExecutionRpc;

pub struct ApiState<N: NetworkSpec, R: ExecutionRpc<N>> {
    pub rpc: Arc<R>,
    _marker: PhantomData<N>,
}

impl<N: NetworkSpec, R: ExecutionRpc<N>> Clone for ApiState<N, R> {
    fn clone(&self) -> Self {
        Self {
            rpc: self.rpc.clone(),
            _marker: PhantomData::default(),
        }
    }
}

impl<N: NetworkSpec, R: ExecutionRpc<N>> ApiState<N, R> {
    pub fn new(rpc: &str) -> Result<Self> {
        Ok(Self {
            rpc: Arc::new(ExecutionRpc::new(rpc)?),
            _marker: PhantomData::default(),
        })
    }

    pub fn new_from_rpc(rpc: Arc<R>) -> Self {
        Self {
            rpc,
            _marker: PhantomData::default(),
        }
    }
}
