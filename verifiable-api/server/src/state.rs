use helios_common::network_spec::NetworkSpec;
use helios_core::execution::rpc::ExecutionRpc;

use crate::service::ApiService;

#[derive(Clone)]
pub struct ApiState<N: NetworkSpec> {
    pub api_service: ApiService<N>,
}
