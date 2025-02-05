use std::marker::PhantomData;

use eyre::Result;

use helios_core::execution::errors::ExecutionError;
use helios_core::execution::rpc::ExecutionRpc;
use helios_core::network_spec::NetworkSpec;

pub struct ExecutionClient<N: NetworkSpec, R: ExecutionRpc<N>> {
    pub rpc: R,
    url: String,
    _marker: PhantomData<N>,
}

impl<N: NetworkSpec, R: ExecutionRpc<N>> Clone for ExecutionClient<N, R> {
    fn clone(&self) -> Self {
        Self::new(&self.url).unwrap()
    }
}

impl<N: NetworkSpec, R: ExecutionRpc<N>> ExecutionClient<N, R> {
    pub fn new(rpc: &str) -> Result<Self> {
        Ok(ExecutionClient::<N, R> {
            rpc: ExecutionRpc::new(rpc)?,
            url: rpc.to_string(),
            _marker: PhantomData::default(),
        })
    }

    pub async fn check_rpc(&self, chain_id: u64) -> Result<()> {
        if self.rpc.chain_id().await? != chain_id {
            Err(ExecutionError::IncorrectRpcNetwork().into())
        } else {
            Ok(())
        }
    }
}
