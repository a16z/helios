use std::{collections::HashMap, path::PathBuf};

use figment::{providers::Serialized, value::Value};
use serde::Serialize;

/// Cli Config
#[derive(Serialize)]
pub struct CliConfig {
    pub execution_rpc: Option<String>,
    pub consensus_rpc: Option<String>,
    pub checkpoint: Option<Vec<u8>>,
    pub rpc_port: Option<u16>,
    pub data_dir: PathBuf,
}

impl CliConfig {
    pub fn as_provider(&self, network: &str) -> Serialized<HashMap<&str, Value>> {
        let mut user_dict = HashMap::new();

        if let Some(rpc) = &self.execution_rpc {
            user_dict.insert("execution_rpc", Value::from(rpc.clone()));
        }

        if let Some(rpc) = &self.consensus_rpc {
            user_dict.insert("consensus_rpc", Value::from(rpc.clone()));
        }

        if let Some(checkpoint) = &self.checkpoint {
            user_dict.insert("checkpoint", Value::from(hex::encode(checkpoint)));
        }

        if let Some(port) = self.rpc_port {
            user_dict.insert("rpc_port", Value::from(port));
        }

        user_dict.insert("data_dir", Value::from(self.data_dir.to_str().unwrap()));

        Serialized::from(user_dict, network)
    }
}
