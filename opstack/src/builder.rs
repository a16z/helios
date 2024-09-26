use std::net::{IpAddr, SocketAddr};

use alloy::primitives::Address;
use eyre::Result;

use crate::{consensus::ConsensusClient, OpStackClient};

#[derive(Default)]
pub struct OpStackClientBuilder {
    chain_id: Option<u64>,
    unsafe_signer: Option<Address>,
    server_rpc: Option<String>,
    execution_rpc: Option<String>,
    rpc_bind_ip: Option<IpAddr>,
    rpc_port: Option<u16>,
}

impl OpStackClientBuilder {
    pub fn new() -> Self {
        OpStackClientBuilder::default()
    }

    pub fn chain_id(mut self, chain_id: u64) -> Self {
        self.chain_id = Some(chain_id);
        self
    }

    pub fn unsafe_signer(mut self, signer: Address) -> Self {
        self.unsafe_signer = Some(signer);
        self
    }

    pub fn server_rpc(mut self, server_rpc: &str) -> Self {
        self.server_rpc = Some(server_rpc.to_string());
        self
    }

    pub fn execution_rpc(mut self, execution_rpc: &str) -> Self {
        self.execution_rpc = Some(execution_rpc.to_string());
        self
    }

    pub fn rpc_bind_ip(mut self, ip: IpAddr) -> Self {
        self.rpc_bind_ip = Some(ip);
        self
    }

    pub fn rpc_port(mut self, port: u16) -> Self {
        self.rpc_port = Some(port);
        self
    }

    pub fn build(self) -> Result<OpStackClient> {
        let Some(chain_id) = self.chain_id else {
            eyre::bail!("chain id required");
        };

        let Some(unsafe_signer) = self.unsafe_signer else {
            eyre::bail!("unsafe signer required");
        };

        let Some(server_rpc) = self.server_rpc else {
            eyre::bail!("server rpc required");
        };

        let Some(execution_rpc) = self.execution_rpc else {
            eyre::bail!("execution rpc required");
        };

        let Some(rpc_bind_ip) = self.rpc_bind_ip else {
            eyre::bail!("rpc bind ip required");
        };

        let Some(rpc_port) = self.rpc_port else {
            eyre::bail!("rpc port required");
        };

        let rpc_address = SocketAddr::new(rpc_bind_ip, rpc_port);
        let consensus = ConsensusClient::new(&server_rpc, unsafe_signer, chain_id);
        OpStackClient::new(&execution_rpc, consensus, Some(rpc_address))
    }
}
