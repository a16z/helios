#[cfg(not(target_arch = "wasm32"))]
use std::net::SocketAddr;
use std::{ops::Deref, sync::Arc};

#[cfg(not(target_arch = "wasm32"))]
use futures::future::pending;
use helios_common::{
    execution_provider::ExecutionProivder, fork_schedule::ForkSchedule, network_spec::NetworkSpec,
};

use crate::consensus::Consensus;
#[cfg(not(target_arch = "wasm32"))]
use crate::jsonrpc;

use self::{api::HeliosApi, node::Node};

pub mod api;
pub mod node;

pub struct HeliosClient<N: NetworkSpec> {
    inner: Arc<dyn HeliosApi<N>>,
}

impl<N: NetworkSpec> HeliosClient<N> {
    pub fn new<C: Consensus<N::BlockResponse>, E: ExecutionProivder<N>>(
        consensus: C,
        execution: E,
        fork_schedule: ForkSchedule,
        #[cfg(not(target_arch = "wasm32"))] rpc_address: Option<SocketAddr>,
    ) -> Self {
        let inner = Arc::new(Node::new(consensus, execution, fork_schedule));

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(rpc_address) = rpc_address {
            let inner_ref = inner.clone();
            tokio::spawn(async move {
                let _handle = jsonrpc::start(inner_ref, rpc_address).await;
                let () = pending().await;
            });
        }

        Self { inner }
    }
}

impl<N: NetworkSpec> Deref for HeliosClient<N> {
    type Target = Arc<dyn HeliosApi<N>>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
