use std::{net::SocketAddr, sync::Arc, time::Duration};

use alloy::primitives::Address;
use axum::{extract::State, routing::get, Json, Router};
use eyre::Result;
use tokio::{
    sync::{mpsc::Receiver, RwLock},
    time::sleep,
};

use crate::{types::ExecutionPayload, SequencerCommitment};

use self::net::{block_handler::BlockHandler, gossip::GossipService};

pub mod net;

pub async fn start_server(
    server_addr: SocketAddr,
    gossip_addr: SocketAddr,
    chain_id: u64,
    signer: Address,
) -> Result<()> {
    let state = Arc::new(RwLock::new(ServerState::new(
        gossip_addr,
        chain_id,
        signer,
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

struct ServerState {
    commitment_recv: Receiver<SequencerCommitment>,
    latest_commitment: Option<(SequencerCommitment, u64)>,
}

impl ServerState {
    pub fn new(addr: SocketAddr, chain_id: u64, signer: Address) -> Result<Self> {
        let (handler, commitment_recv) = BlockHandler::new(signer, chain_id);
        let gossip = GossipService::new(addr, chain_id, handler);
        gossip.start()?;

        Ok(Self {
            commitment_recv,
            latest_commitment: None,
        })
    }

    pub fn update(&mut self) {
        if let Ok(commitment) = self.commitment_recv.try_recv() {
            if let Ok(payload) = ExecutionPayload::try_from(&commitment) {
                if self.is_latest_commitment(payload.block_number) {
                    tracing::info!("new commitment for block: {}", payload.block_number);
                    self.latest_commitment = Some((commitment, payload.block_number));
                }
            }
        }
    }

    fn is_latest_commitment(&self, block_number: u64) -> bool {
        if let Some((_, latest_block_number)) = self.latest_commitment {
            block_number > latest_block_number
        } else {
            true
        }
    }
}
