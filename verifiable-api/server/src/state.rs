use helios_common::network_spec::NetworkSpec;

use crate::service::ApiService;

#[derive(Clone)]
pub struct ApiState<N: NetworkSpec> {
    pub api_service: ApiService<N>,
}
