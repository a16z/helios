use discv5::{
    enr::NodeId,
    Discv5, Enr, Discv5Event, Discv5Error, QueryError,
};
use libp2p::{
    identity::Keypair, 
    swarm::{NetworkBehaviour, NetworkBehaviourAction, PollParameters},
    PeerId, Multiaddr,
    multiaddr::Protocol,
    futures::FutureExt,
};
use tokio::sync::mpsc;
use futures::{
    stream::FuturesUnordered,
    StreamExt,
};
use std::future::Future;
use std::pin::Pin;
use std::net::SocketAddr;
use std::time::Instant;
use std::collections::HashMap;
use std::task::{Context, Poll};
use log::{debug, error};

use super::config::Config as ConsensusConfig;
mod enr;
use enr::{key_from_libp2p, build_enr, EnrForkId, EnrAsPeerId};

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

pub enum DiscoveryError {
    Discv5Error(Discv5Error),
    UnexpectedError(String),
}

impl From<&str> for DiscoveryError {
    fn from(e: &str) -> Self {
        DiscoveryError::UnexpectedError(e.to_string())
    }
}

impl From<String> for DiscoveryError {
    fn from(e: String) -> Self {
        DiscoveryError::UnexpectedError(e)
    }
}

impl From<Discv5Error> for DiscoveryError {
    fn from(e: Discv5Error) -> Self {
        DiscoveryError::Discv5Error(e)
    }
}

pub struct Discovery {
    discv5: Discv5,
    local_enr: Enr,
    event_stream: EventStream,
    multiaddr_map: HashMap<PeerId, Multiaddr>,
    active_queries: FuturesUnordered<std::pin::Pin<Box<dyn Future<Output = DiscResult> + Send>>>,
    pub started: bool,
}

impl Discovery {
    pub async fn new(
        local_key: &Keypair,
        config: ConsensusConfig,
    ) -> Result<Self, DiscoveryError> {
        let enr_key = key_from_libp2p(local_key)?;
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
                .map_err(|e| e.to_string())?;
            debug!("Discovery started");
            EventStream::Awaiting(Box::pin(discv5.event_stream()))
        } else {
            EventStream::InActive
        };

        Ok(Self {
            discv5,
            local_enr,
            event_stream,
            multiaddr_map: HashMap::new(),
            active_queries: FuturesUnordered::new(),
            started: !config.disable_discovery,
        })
    }

    fn find_peers(&mut self) {
        let fork_digest = self.local_enr.fork_id().unwrap().fork_digest;

        let predicate: Box<dyn Fn(&Enr) -> bool + Send> = Box::new(move |enr: &Enr| {
            enr.fork_id().map(|e| e.fork_digest) == Ok(fork_digest.clone()) && enr.tcp4().is_some()
        });

        let target = NodeId::random();

        let peers_enr = self.discv5.find_node_predicate(target, predicate, 16);

        self.active_queries.push(Box::pin(peers_enr));
    }

    fn get_peers(&mut self, cx: &mut Context) -> Option<DiscoveredPeers> {
        while let Poll::Ready(Some(res)) = self.active_queries.poll_next_unpin(cx) {
            if res.is_ok() {
                self.active_queries = FuturesUnordered::new();

                let mut peers: HashMap<PeerId, Option<Instant>> = HashMap::new();

                for peer_enr in res.unwrap() {
                    let peer_id = peer_enr.clone().as_peer_id();

                    if peer_enr.ip4().is_some() && peer_enr.tcp4().is_some() {
                        let mut multiaddr: Multiaddr = peer_enr.ip4().unwrap().into();

                        multiaddr.push(Protocol::Tcp(peer_enr.tcp4().unwrap()));

                        self.multiaddr_map.insert(peer_id, multiaddr);
                    }

                    peers.insert(peer_id, None);
                }

                return Some(DiscoveredPeers { peers });
            }
        }

        None
    }    
}

#[derive(Debug, Clone)]
pub struct DiscoveredPeers {
    pub peers: HashMap<PeerId, Option<Instant>>,
}

impl NetworkBehaviour for Discovery {
    type ConnectionHandler = libp2p::swarm::dummy::ConnectionHandler;
    type OutEvent = DiscoveredPeers;

    fn new_handler(&mut self) -> Self::ConnectionHandler {
        Self::ConnectionHandler {}
    }

    fn addresses_of_peer(&mut self, peer_id: &PeerId) -> Vec<Multiaddr> {
        let mut peer_addresses = Vec::new();

        if let Some(address) = self.multiaddr_map.get(peer_id) {
            peer_addresses.push(address.clone());
        }

        peer_addresses
    }

    fn poll(
        &mut self,
        cx: &mut Context,
        _: &mut impl PollParameters,
    ) -> Poll<NetworkBehaviourAction<Self::OutEvent, Self::ConnectionHandler>> {
        if !self.started {
            self.started = true;
            self.find_peers();

            return Poll::Pending;
        }

        if let Some(dp) = self.get_peers(cx) {
            return Poll::Ready(NetworkBehaviourAction::GenerateEvent(dp));
        }
        // Process the discovery server event stream
        match self.event_stream {
            EventStream::Awaiting(ref mut fut) => {
                // Still awaiting the event stream, poll it
                if let Poll::Ready(event_stream) = fut.poll_unpin(cx) {
                    match event_stream {
                        Ok(stream) => {
                            println!("Discv5 event stream ready");
                            self.event_stream = EventStream::Present(stream);
                        }
                        Err(_) => {
                            println!("Discv5 event stream failed");
                            self.event_stream = EventStream::InActive;
                        }
                    }
                }
            }
            EventStream::InActive => {}
            EventStream::Present(ref mut stream) => {
                while let Poll::Ready(Some(event)) = stream.poll_recv(cx) {
                    match event {
                        Discv5Event::SessionEstablished(_enr, _) => {
                            // println!("Session Established: {:?}", enr);
                        }
                        _ => (),
                    }
                }
            }
        }
        Poll::Pending
    }
}
