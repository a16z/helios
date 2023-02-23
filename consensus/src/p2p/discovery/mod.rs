use discv5::{
    Discv5, Enr, Discv5Event, Discv5Error, QueryError,
};
use libp2p::{
    identity::Keypair,
};
use tokio::sync::mpsc;
use futures::stream::FuturesUnordered;
use std::future::Future;
use std::pin::Pin;
use std::net::SocketAddr;
use log::{debug, error};

use super::config::Config as ConsensusConfig;
mod enr;
use enr::{key_from_libp2p, build_enr};

enum EventStream {
    Present(mpsc::Receiver<Discv5Event>),
    InActive,
    Awaiting(
        Pin<
            Box<
                dyn Future<Output = Result<mpsc::Receiver<Discv5Event>, Discv5Error>>
                    + Send,
            >,
        >,
    ),
}

type DiscResult = Result<Vec<Enr>, QueryError>;

enum DiscoveryError {
    Discv5Error(Discv5Error),
    BuildEnrError(String),
}

pub struct Discovery {
    discv5: Discv5,
    local_enr: Enr,
    event_stream: EventStream,
    active_queries: FuturesUnordered<std::pin::Pin<Box<dyn Future<Output = DiscResult> + Send>>>,
    pub started: bool,
}

impl Discovery {
    pub async fn new(
        local_key: &Keypair,
        config: ConsensusConfig,
    ) -> Result<Self, Discv5Error> {
        let enr_key = key_from_libp2p(local_key).map_err(|e| {
            error!("Failed to build ENR key: {:?}", e);
            DiscoveryError::InvalidKey
        })?;
        let local_enr = build_enr(&enr_key, &config);
        let listen_socket = SocketAddr::new(config.listen_addr, config.discovery_port);

        let mut discv5 = Discv5::new(local_enr.clone(), enr_key, config.discv5_config)?;

        for boot_node_enr in config.boot_nodes_enr.clone() {
            debug!("Adding boot node: {:?}", boot_node_enr);
            let repr = boot_node_enr.to_string();
            let _ = discv5.add_enr(boot_node_enr).map_err(|e| {
                error!("Failed to add boot node: {:?}, {:?}", repr, e);
            });
        }
        let event_stream = if !config.disable_discovery {
            discv5
                .start(listen_socket)
                .await
                .map_err(|e| e.to_string());
            debug!("Discovery started");
            EventStream::Awaiting(Box::pin(discv5.event_stream()))
        } else {
            EventStream::InActive
        };

        Ok(Self {
            discv5,
            local_enr,
            event_stream,
            active_queries: FuturesUnordered::new(),
            started: !config.disable_discovery,
        })
    }
}
