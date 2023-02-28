use eyre::Result;
use crate::types::{BeaconBlock, Bootstrap, FinalityUpdate, OptimisticUpdate, Update};
use async_trait::async_trait;
use libp2p::swarm::NetworkBehaviour;

use crate::p2p::discovery::Discovery;
use crate::rpc::ConsensusNetworkInterface;

pub struct P2pNetworkInterface {
    discovery: Discovery,
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl ConsensusNetworkInterface for P2pNetworkInterface {
    fn new(_path: &str) -> Self {
        P2pNetworkInterface {}
    }

    async fn get_bootstrap(&self, _block_root: &'_ [u8]) -> Result<Bootstrap> {
        unimplemented!()
    }

    async fn get_updates(&self, _period: u64, _count: u8) -> Result<Vec<Update>> {
        unimplemented!()
    }

    async fn get_finality_update(&self) -> Result<FinalityUpdate> {
        unimplemented!()
    }

    async fn get_optimistic_update(&self) -> Result<OptimisticUpdate> {
        unimplemented!()
    }

    async fn get_block(&self, _slot: u64) -> Result<BeaconBlock> {
        unimplemented!()
    }

    async fn chain_id(&self) -> Result<u64> {
        unimplemented!()
    }
}

#[derive(NetworkBehaviour)]
pub struct Behaviour {
    discovery: Discovery,
}
