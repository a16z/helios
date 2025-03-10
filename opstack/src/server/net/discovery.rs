use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};

use alloy::{
    primitives::Bytes,
    rlp::{self, Decodable},
};
use discv5::{
    enr::{Builder, CombinedKey, Enr, NodeId},
    ConfigBuilder, Discv5, ListenConfig,
};
use eyre::Result;
use tokio::{
    sync::mpsc::{self, Receiver},
    time::sleep,
};
use unsigned_varint::{decode, encode};

use super::bootnodes::bootnodes;

/// Starts the [Discv5] discovery service and continually tries to find new peers.
/// Returns a [Receiver] to receive [Peer] structs
pub fn start(addr: SocketAddr, chain_id: u64) -> Result<Receiver<SocketAddr>> {
    let bootnodes = bootnodes();
    let mut disc = create_disc(addr, chain_id)?;

    let (sender, recv) = mpsc::channel::<SocketAddr>(256);

    tokio::spawn(async move {
        bootnodes.into_iter().for_each(|enr| {
            disc.add_enr(enr);
        });
        disc.start().await.unwrap();

        tracing::info!("started peer discovery");

        loop {
            let target = NodeId::random();
            match disc.find_node(target).await {
                Ok(nodes) => {
                    let peers = nodes
                        .iter()
                        .filter(|node| is_valid_node(node, chain_id))
                        .flat_map(|peer| {
                            if let Some(ip) = peer.ip4() {
                                return Some(SocketAddr::new(IpAddr::V4(ip), peer.tcp4().unwrap()));
                            }

                            if let Some(ip) = peer.ip6() {
                                return Some(SocketAddr::new(IpAddr::V6(ip), peer.tcp6().unwrap()));
                            }

                            None
                        });

                    for peer in peers {
                        _ = sender.send(peer).await;
                    }
                }
                Err(err) => {
                    tracing::warn!("discovery error: {:?}", err);
                }
            }

            sleep(Duration::from_secs(1)).await;
        }
    });

    Ok(recv)
}

/// Returns `true` if a node [Enr] contains an `opstack` key and is on the same network.
fn is_valid_node(node: &Enr<CombinedKey>, chain_id: u64) -> bool {
    node.get_raw_rlp("opstack")
        .map(|opstack| {
            OpStackEnrData::try_from(opstack)
                .map(|opstack| opstack.chain_id == chain_id && opstack.version == 0)
                .unwrap_or_default()
        })
        .unwrap_or_default()
}

/// Generates an [Enr] and creates a [Discv5] service struct
fn create_disc(addr: SocketAddr, chain_id: u64) -> Result<Discv5> {
    let opstack = OpStackEnrData {
        chain_id,
        version: 0,
    };
    let opstack_data: Vec<u8> = opstack.into();

    let listen_config = ListenConfig::from(addr);
    let config = ConfigBuilder::new(listen_config).build();
    let key = CombinedKey::generate_secp256k1();
    let enr = Builder::default()
        .add_value_rlp("opstack", opstack_data)
        .build(&key)?;

    Discv5::new(enr, key, config).map_err(|_| eyre::eyre!("could not create disc service"))
}

/// The unique L2 network identifier
#[derive(Debug)]
struct OpStackEnrData {
    /// Chain ID
    chain_id: u64,
    /// The version. Always set to 0.
    version: u64,
}

impl TryFrom<&[u8]> for OpStackEnrData {
    type Error = eyre::Report;

    /// Converts a slice of RLP encoded bytes to Op Stack Enr Data.
    fn try_from(value: &[u8]) -> Result<Self> {
        let mut buffer = value;
        let bytes = Bytes::decode(&mut buffer)?;
        let (chain_id, rest) = decode::u64(&bytes)?;
        let (version, _) = decode::u64(rest)?;

        Ok(Self { chain_id, version })
    }
}

impl From<OpStackEnrData> for Vec<u8> {
    /// Converts Op Stack Enr data to a vector of bytes.
    fn from(value: OpStackEnrData) -> Vec<u8> {
        let mut chain_id_buf = encode::u128_buffer();
        let chain_id_slice = encode::u128(value.chain_id as u128, &mut chain_id_buf);

        let mut version_buf = encode::u128_buffer();
        let version_slice = encode::u128(value.version as u128, &mut version_buf);

        let opstack = [chain_id_slice, version_slice].concat();

        rlp::encode(&opstack).to_vec()
    }
}
