use std::{net::SocketAddr, sync::Arc, time::Duration};

use alloy::{
    primitives::{Address, FixedBytes, B256},
    rpc::types::EIP1186AccountProofResponse,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use eyre::Result;
use libp2p::Multiaddr;
use op_alloy_rpc_types_engine::{OpExecutionPayload, OpNetworkPayloadEnvelope};
use tokio::{
    sync::{
        mpsc::{channel, Receiver},
        RwLock,
    },
    time::sleep,
};
use url::Url;
use kona_p2p::NetworkBuilder;
use ssz::Encode;

use crate::{types::ExecutionPayload, SequencerCommitment};

// use self::net::{block_handler::BlockHandler, gossip::GossipService};
use helios_core::execution::rpc::{http_rpc::HttpRpc, ExecutionRpc};
use helios_ethereum::spec::Ethereum;
use std::str::FromStr;

pub mod net;
mod poller;

// Storage slot containing the unsafe signer address in all superchain system config contracts
const UNSAFE_SIGNER_SLOT: &str =
    "0x65a7ed542fb37fe237fdfbdd70b31598523fe5b32879e307bae27a0bd9581c08";

pub async fn start_server(
    server_addr: SocketAddr,
    gossip_addr: SocketAddr,
    chain_id: u64,
    signer: Address,
    system_config_contract: Address,
    replica_urls: Vec<Url>,
    execution_rpc: Url,
) -> Result<()> {
    let state = Arc::new(RwLock::new(ServerState::new(
        gossip_addr,
        chain_id,
        signer,
        system_config_contract,
        replica_urls,
        execution_rpc,
    )?));

    let state_copy = state.clone();
    let _handle = tokio::spawn(async move {
        loop {
            state_copy.write().await.update();
            sleep(Duration::from_secs(1)).await;
        }
    });

    let router = Router::new()
        .route("/latest", get(latest_handler))
        .route("/chain_id", get(chain_id_handler))
        .route(
            "/unsafe_signer_proof/:block_hash",
            get(unsafe_signer_proof_handler),
        )
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(server_addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}

async fn latest_handler(
    State(state): State<Arc<RwLock<ServerState>>>,
) -> Json<Option<SequencerCommitment>> {
    Json(state.read().await.latest_commitment.clone().map(|v| v.0))
}

async fn chain_id_handler(State(state): State<Arc<RwLock<ServerState>>>) -> Json<u64> {
    Json(state.read().await.chain_id)
}

async fn unsafe_signer_proof_handler(
    State(state): State<Arc<RwLock<ServerState>>>,
    Path(block_hash): Path<FixedBytes<32>>,
) -> Result<Json<EIP1186AccountProofResponse>, StatusCode> {
    let rpc = HttpRpc::<Ethereum>::new(state.read().await.execution_rpc.as_ref())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let signer_slot =
        B256::from_str(UNSAFE_SIGNER_SLOT).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let proof = rpc
        .get_proof(
            state.read().await.system_config_contract,
            &[signer_slot],
            block_hash.into(),
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(proof))
}
struct ServerState {
    chain_id: u64,
    block_recv: Receiver<OpNetworkPayloadEnvelope>,
    latest_commitment: Option<(SequencerCommitment, u64)>,
    execution_rpc: Url,
    system_config_contract: Address,
}

impl ServerState {
    pub fn new(
        addr: SocketAddr,
        chain_id: u64,
        signer: Address,
        system_config_contract: Address,
        _replica_urls: Vec<Url>,
        execution_rpc: Url,
    ) -> Result<Self> {
        let mut gossip_addr = Multiaddr::from(addr.ip());
        gossip_addr.push(libp2p::multiaddr::Protocol::Tcp(addr.port()));
        let mut network = NetworkBuilder::new()
            .with_chain_id(chain_id)
            .with_unsafe_block_signer(signer)
            .with_discovery_address("0.0.0.0:44491".parse().unwrap())
            .with_gossip_address(gossip_addr)
            .build()?;

        let block_recv = network.take_unsafe_block_recv().unwrap();
        network.start()?;

        Ok(Self {
            chain_id,
            block_recv,
            latest_commitment: None,
            execution_rpc,
            system_config_contract,
        })
    }

    pub fn update(&mut self) {
        while let Ok(block) = self.block_recv.try_recv() {
            let block_number = block.payload.block_number();
            let commitment = encode_payload_envelope(block);
            if self.is_latest_commitment(block_number) {
                tracing::info!("new commitment for block: {}", block_number);
                self.latest_commitment = Some((commitment, block_number));
            }
        }
        println!("finished update");
    }

    fn is_latest_commitment(&self, block_number: u64) -> bool {
        if let Some((_, latest_block_number)) = self.latest_commitment {
            block_number > latest_block_number
        } else {
            true
        }
    }
}

fn encode_payload_envelope(value: OpNetworkPayloadEnvelope) -> SequencerCommitment {
    let parent_root = value.parent_beacon_block_root.unwrap();
    let payload = match value.payload {
        OpExecutionPayload::V1(value) => value.as_ssz_bytes(),
        OpExecutionPayload::V2(value) => value.as_ssz_bytes(),
        OpExecutionPayload::V3(value) => value.as_ssz_bytes(),
        OpExecutionPayload::V4(value) => value.as_ssz_bytes(),
    };

    let data = [parent_root.as_slice(), &payload].concat();

    SequencerCommitment {
        data: data.into(),
        signature: value.signature,
    }
}
