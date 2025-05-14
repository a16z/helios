use std::ops::Deref;

use helios_common::{fork_schedule::ForkSchedule, network_spec::NetworkSpec};

use crate::{consensus::Consensus, execution::providers::ExecutionProivder};

use self::{api::HeliosApi, node::Node};

#[cfg(not(target_arch = "wasm32"))]
pub mod api;
pub mod node;

pub struct HeliosClient<N: NetworkSpec> {
    inner: Box<dyn HeliosApi<N>>,
}

impl<N: NetworkSpec> HeliosClient<N> {
    pub fn new<C: Consensus<N::BlockResponse>, E: ExecutionProivder<N>>(
        consensus: C,
        execution: E,
        fork_schedule: ForkSchedule,
    ) -> Self {
        Self {
            inner: Box::new(Node::new(consensus, execution, fork_schedule)),
        }
    }
}

impl<N: NetworkSpec> Deref for HeliosClient<N> {
    type Target = Box<dyn HeliosApi<N>>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
