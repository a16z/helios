#[derive(Clone)]
pub enum ExecutionMode {
    Rpc(String),
    VerifiableApi(String),
}

impl ExecutionMode {
    pub fn from_urls(rpc: Option<String>, verifiable_api: Option<String>) -> Self {
        match (rpc, verifiable_api) {
            (None, Some(verifiable_api)) => Self::VerifiableApi(verifiable_api),
            (Some(rpc), None) => Self::Rpc(rpc),
            (Some(_), Some(_)) => {
                panic!("Cannot specify both execution_rpc and execution_verifiable_api")
            }
            (None, None) => panic!("Must specify either execution_rpc or execution_verifiable_api"),
        }
    }
}
