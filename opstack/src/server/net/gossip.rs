use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};

use eyre::Result;
use libp2p::{
    futures::StreamExt,
    gossipsub::{self, IdentTopic, Message, MessageId},
    mplex::MplexConfig,
    multiaddr::Protocol,
    noise, ping,
    swarm::{NetworkBehaviour, SwarmBuilder, SwarmEvent},
    tcp, Multiaddr, PeerId, Swarm, Transport,
};
use libp2p_identity::Keypair;
use sha2::{Digest, Sha256};
use tokio::select;

use super::{block_handler::BlockHandler, discovery};

/// OP Stack gossip service
pub struct GossipService {
    /// The socket address that the service is listening on.
    addr: SocketAddr,
    /// The chain ID of the network
    chain_id: u64,
    /// A unique keypair to validate the node's identity
    keypair: Option<Keypair>,
    /// Handler for the block
    block_handler: BlockHandler,
}

impl GossipService {
    /// Creates a new [Service]
    pub fn new(addr: SocketAddr, chain_id: u64, handler: BlockHandler) -> Self {
        Self {
            addr,
            chain_id,
            keypair: None,
            block_handler: handler,
        }
    }

    /// Sets the keypair for [Service]
    pub fn set_keypair(mut self, keypair: Keypair) -> Self {
        self.keypair = Some(keypair);
        self
    }

    /// Starts the Discv5 peer discovery & libp2p services
    /// and continually listens for new peers and messages to handle
    pub fn start(self) -> Result<()> {
        let keypair = self.keypair.unwrap_or_else(Keypair::generate_secp256k1);

        let mut swarm = create_swarm(keypair, &self.block_handler)?;
        let mut peer_recv = discovery::start(self.addr, self.chain_id)?;
        let multiaddr = socket_to_multiaddr(self.addr);

        swarm
            .listen_on(multiaddr)
            .map_err(|_| eyre::eyre!("swarm listen failed"))?;

        tokio::spawn(async move {
            loop {
                select! {
                    peer = peer_recv.recv() => {
                        if let Some(peer) = peer {
                            tracing::info!("adding peer");
                            let peer = socket_to_multiaddr(peer);
                            _ = swarm.dial(peer);
                        }
                    },
                    event = swarm.select_next_some() => {
                        if let SwarmEvent::Behaviour(event) = event {
                            event.handle(&mut swarm, &self.block_handler);
                        }
                    },
                }
            }
        });

        Ok(())
    }
}

fn socket_to_multiaddr(socket: SocketAddr) -> Multiaddr {
    let mut multiaddr = Multiaddr::empty();
    match socket.ip() {
        IpAddr::V4(ip) => multiaddr.push(Protocol::Ip4(ip)),
        IpAddr::V6(ip) => multiaddr.push(Protocol::Ip6(ip)),
    }
    multiaddr.push(Protocol::Tcp(socket.port()));
    multiaddr
}

/// Maximum decompressed size of a gossip message, matching op-node's `maxGossipSize`.
const MAX_GOSSIP_SIZE: usize = 10 * (1 << 20);

/// Computes the message ID of a `gossipsub` message.
///
/// Mirrors op-node's `BuildMsgIdFn` (op-node/p2p/gossip.go): the ID is the first 20 bytes of
/// `sha256(domain || uint64_le(len(topic)) || topic || data)`. If the message is valid snappy
/// within [`MAX_GOSSIP_SIZE`], `data` is the decompressed payload under the valid-snappy domain.
/// Otherwise it is the raw payload under the invalid-snappy domain.
fn compute_message_id(msg: &Message) -> MessageId {
    let decompressed = snap::raw::decompress_len(&msg.data)
        .ok()
        .filter(|&len| len <= MAX_GOSSIP_SIZE)
        .and_then(|_| snap::raw::Decoder::new().decompress_vec(&msg.data).ok());

    let (domain, data) = match &decompressed {
        Some(data) => ([0x1, 0x0, 0x0, 0x0], data.as_slice()),
        None => ([0x0, 0x0, 0x0, 0x0], msg.data.as_slice()),
    };
    let topic = msg.topic.as_str().as_bytes();

    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((topic.len() as u64).to_le_bytes());
    hasher.update(topic);
    hasher.update(data);

    MessageId(hasher.finalize()[..20].to_vec())
}

/// Creates the libp2p [Swarm]
fn create_swarm(keypair: Keypair, handler: &BlockHandler) -> Result<Swarm<Behaviour>> {
    let transport = tcp::tokio::Transport::new(tcp::Config::default())
        .upgrade(libp2p::core::upgrade::Version::V1Lazy)
        .authenticate(noise::Config::new(&keypair)?)
        .multiplex(MplexConfig::default())
        .boxed();

    let behaviour = Behaviour::new(handler)?;

    Ok(
        SwarmBuilder::with_tokio_executor(transport, behaviour, PeerId::from(keypair.public()))
            .build(),
    )
}

/// Specifies the [NetworkBehaviour] of the node
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "Event")]
struct Behaviour {
    /// Adds [libp2p::ping] to respond to inbound pings, and send periodic outbound pings
    ping: ping::Behaviour,
    /// Adds [libp2p::gossipsub] to enable gossipsub as the routing layer
    gossipsub: gossipsub::Behaviour,
}

impl Behaviour {
    /// Configures the swarm behaviors, subscribes to the gossip topics, and returns a new [Behaviour]
    fn new(handler: &BlockHandler) -> Result<Self> {
        let ping = ping::Behaviour::default();

        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .mesh_n(8)
            .mesh_n_low(6)
            .mesh_n_high(12)
            .gossip_lazy(6)
            .heartbeat_interval(Duration::from_millis(500))
            .fanout_ttl(Duration::from_secs(24))
            .history_length(12)
            .history_gossip(3)
            .duplicate_cache_time(Duration::from_secs(65))
            .validation_mode(gossipsub::ValidationMode::None)
            .validate_messages()
            .message_id_fn(compute_message_id)
            .build()
            .map_err(|_| eyre::eyre!("gossipsub config creation failed"))?;

        let mut gossipsub =
            gossipsub::Behaviour::new(gossipsub::MessageAuthenticity::Anonymous, gossipsub_config)
                .map_err(|_| eyre::eyre!("gossipsub behaviour creation failed"))?;

        handler
            .topics()
            .iter()
            .map(|topic| {
                let topic = IdentTopic::new(topic.to_string());
                gossipsub
                    .subscribe(&topic)
                    .map_err(|_| eyre::eyre!("subscription failed"))
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Self { ping, gossipsub })
    }
}

/// The type of message received
enum Event {
    /// Represents a [ping::Event]
    #[allow(dead_code)]
    Ping(ping::Event),
    /// Represents a [gossipsub::Event]
    Gossipsub(gossipsub::Event),
}

impl Event {
    /// Handles received gossipsub messages. Ping messages are ignored.
    /// Reports back to [libp2p::gossipsub] to apply peer scoring and forward the message to other peers if accepted.
    fn handle(self, swarm: &mut Swarm<Behaviour>, handler: &BlockHandler) {
        if let Self::Gossipsub(gossipsub::Event::Message {
            propagation_source,
            message_id,
            message,
        }) = self
        {
            let status = handler.handle(message);

            _ = swarm
                .behaviour_mut()
                .gossipsub
                .report_message_validation_result(&message_id, &propagation_source, status);
        }
    }
}

impl From<ping::Event> for Event {
    /// Converts [ping::Event] to [Event]
    fn from(value: ping::Event) -> Self {
        Event::Ping(value)
    }
}

impl From<gossipsub::Event> for Event {
    /// Converts [gossipsub::Event] to [Event]
    fn from(value: gossipsub::Event) -> Self {
        Event::Gossipsub(value)
    }
}

#[cfg(test)]
mod tests {
    use libp2p::gossipsub::TopicHash;

    use super::*;

    fn message(data: Vec<u8>, topic: &str) -> Message {
        Message {
            source: None,
            data,
            sequence_number: None,
            topic: TopicHash::from_raw(topic),
        }
    }

    // Reference vectors from kona's `compute_message_id_matches_op_node_golden_vectors`
    // (optimism repo, rust/kona/crates/node/gossip/src/config.rs), which mirrors
    // op-node's `BuildMsgIdFn` in op-node/p2p/gossip.go.

    #[test]
    fn test_compute_message_id_valid_snappy() {
        let data = snap::raw::Encoder::new()
            .compress_vec(&[1, 2, 3, 4, 5])
            .unwrap();
        let id = compute_message_id(&message(data, "test"));
        assert_eq!(
            hex::encode(id.0),
            "adbe547b27f41294a08a09210a5e8531e83cdc16"
        );
    }

    #[test]
    fn test_compute_message_id_invalid_snappy() {
        let id = compute_message_id(&message(vec![1, 2, 3, 4, 5], "test"));
        assert_eq!(
            hex::encode(id.0),
            "b6897dcba59347fedcd694cc0f5117093c9dc727"
        );
    }
}
