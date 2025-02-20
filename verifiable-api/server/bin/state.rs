use helios_common::network_spec::NetworkSpec;
use helios_core::execution::rpc::ExecutionRpc;

use crate::api_service::ApiService;

#[derive(Clone)]
pub struct ApiState<N: NetworkSpec, R: ExecutionRpc<N>> {
    pub api_service: ApiService<N, R>,
}
