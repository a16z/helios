use std::{collections::HashMap, path::PathBuf};

use figment::{providers::Serialized, value::Value};
use serde::{Deserialize, Serialize};

/// Cli Config
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct CliConfig {
    pub execution_rpc: Option<String>,
    pub consensus_rpc: Option<String>,
    pub checkpoint: Option<Vec<u8>>,
    pub rpc_port: Option<u16>,
    pub data_dir: PathBuf,
    pub fallback: Option<String>,
    pub load_external_fallback: bool,
    pub strict_checkpoint_age: bool,
    pub p2p_enabled: bool,
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

        if let Some(fallback) = &self.fallback {
            user_dict.insert("fallback", Value::from(fallback.clone()));
        }

        user_dict.insert(
            "load_external_fallback",
            Value::from(self.load_external_fallback),
        );

        user_dict.insert(
            "strict_checkpoint_age",
            Value::from(self.strict_checkpoint_age),
        );

        Serialized::from(user_dict, network)
    }
}
