use std::net::IpAddr;
use std::{collections::HashMap, path::PathBuf};
use url::Url;

use alloy::primitives::B256;
use figment::{providers::Serialized, value::Value};
use serde::{Deserialize, Serialize};

/// Cli Config
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct CliConfig {
    pub execution_rpc: Option<Url>,
    pub verifiable_api: Option<Url>,
    pub consensus_rpc: Option<Url>,
    pub checkpoint: Option<B256>,
    pub rpc_bind_ip: Option<IpAddr>,
    pub rpc_port: Option<u16>,
    pub allowed_origins: Option<Vec<String>>,
    pub data_dir: Option<PathBuf>,
    pub fallback: Option<Url>,
    pub load_external_fallback: Option<bool>,
    pub strict_checkpoint_age: Option<bool>,
}

impl CliConfig {
    pub fn as_provider(&self, network: &str) -> Serialized<HashMap<&str, Value>> {
        let mut user_dict = HashMap::new();

        if let Some(rpc) = &self.execution_rpc {
            user_dict.insert("execution_rpc", Value::from(rpc.to_string()));
        }

        if let Some(api) = &self.verifiable_api {
            user_dict.insert("verifiable_api", Value::from(api.to_string()));
        }

        if let Some(rpc) = &self.consensus_rpc {
            user_dict.insert("consensus_rpc", Value::from(rpc.to_string()));
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

        if let Some(allowed_origins) = &self.allowed_origins {
            user_dict.insert("allowed_origins", Value::from(allowed_origins.clone()));
        }

        if let Some(data_dir) = self.data_dir.as_ref() {
            user_dict.insert("data_dir", Value::from(data_dir.to_str().unwrap()));
        }

        if let Some(fallback) = &self.fallback {
            user_dict.insert("fallback", Value::from(fallback.to_string()));
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
