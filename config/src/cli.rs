use std::net::IpAddr;
use std::{collections::HashMap, path::PathBuf};

use figment::{providers::Serialized, value::Value};
use serde::{Deserialize, Serialize};

/// Cli Config
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct CliConfig {
    pub execution_rpc: Option<String>,
    pub consensus_rpc: Option<String>,
    pub checkpoint: Option<Vec<u8>>,
    pub rpc_bind_ip: Option<IpAddr>,
    pub rpc_port: Option<u16>,
    pub data_dir: Option<PathBuf>,
    pub fallback: Option<String>,
    pub load_external_fallback: Option<bool>,
    pub strict_checkpoint_age: Option<bool>,
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

        if let Some(ip) = self.rpc_bind_ip {
            user_dict.insert("rpc_bind_ip", Value::from(ip.to_string()));
        }

        if let Some(port) = self.rpc_port {
            user_dict.insert("rpc_port", Value::from(port));
        }

        if let Some(data_dir) = self.data_dir.as_ref() {
            user_dict.insert("data_dir", Value::from(data_dir.to_str().unwrap()));
        }

        if let Some(fallback) = &self.fallback {
            user_dict.insert("fallback", Value::from(fallback.clone()));
        }

        if let Some(l) = self.load_external_fallback {
            user_dict.insert("load_external_fallback", Value::from(l));
        }

        if let Some(s) = self.strict_checkpoint_age {
            user_dict.insert("strict_checkpoint_age", Value::from(s));
        }

        Serialized::from(user_dict, network)
    }
}
