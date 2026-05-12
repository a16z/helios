use std::{ops::Deref, sync::Arc};

#[cfg(not(target_arch = "wasm32"))]
use futures::future::pending;
use helios_common::{
    execution_provider::ExecutionProvider, fork_schedule::ForkSchedule, network_spec::NetworkSpec,
};

use crate::consensus::Consensus;
#[cfg(not(target_arch = "wasm32"))]
use crate::jsonrpc::{self, RpcServerConfig};

use self::{api::HeliosApi, node::Node};

pub mod api;
pub mod node;

pub struct HeliosClient<N: NetworkSpec> {
    inner: Arc<dyn HeliosApi<N>>,
}

impl<N: NetworkSpec> HeliosClient<N> {
    pub fn new<C: Consensus<N::BlockResponse>, E: ExecutionProvider<N>>(
        consensus: C,
        execution: E,
        fork_schedule: ForkSchedule,
        #[cfg(not(target_arch = "wasm32"))] rpc_config: Option<RpcServerConfig>,
    ) -> Self {
        let inner = Arc::new(Node::new(consensus, execution, fork_schedule));

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(rpc_config) = rpc_config {
            let inner_ref = inner.clone();
            tokio::spawn(async move {
                let shutdown_ref = inner_ref.clone();
                match jsonrpc::start(inner_ref, rpc_config).await {
                    Ok(_handle) => {
                        let () = pending().await;
                    }
                    Err(err) => {
                        tracing::error!(?err, "failed to start JSON-RPC server");
                        shutdown_ref.shutdown().await;
                    }
                }
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
