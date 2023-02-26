use discv5::{
    enr::{self, CombinedKey, CombinedPublicKey},
    Enr, Discv5Error,
};
use libp2p::identity::Keypair;
use libp2p::PeerId;
use crate::utils::ForkId;
use crate::p2p::config::Config;
use ssz_rs::{Serialize, Deserialize};

pub const ETH2_ENR_KEY: &str = "eth2";

pub fn build_enr(
    key: &CombinedKey,
    config: &Config, 
) -> Enr {
    let mut enr_builder = enr::EnrBuilder::new("v4");
    enr_builder.ip("0.0.0.0".parse().unwrap());

    enr_builder.udp4(9000);

    enr_builder.tcp4(9000);
    let mut bytes = vec![];
    ForkId::default().serialize(&mut bytes).unwrap();
    enr_builder.add_value(ETH2_ENR_KEY, bytes.as_slice());

    enr_builder.build(key).unwrap()
}

pub trait EnrAsPeerId {
    fn as_peer_id(&self) -> PeerId;
}

impl EnrAsPeerId for Enr {
    fn as_peer_id(&self) -> PeerId {
        let public_key = self.public_key();

        match public_key {
            CombinedPublicKey::Secp256k1(pk) => {
                let pk_bytes = pk.to_bytes();
                let libp2p_pk = libp2p::core::PublicKey::Secp256k1(
                    libp2p::core::identity::secp256k1::PublicKey::decode(&pk_bytes)
                        .expect("Failed to decode public key"),
                );
                PeerId::from_public_key(&libp2p_pk)
            }
            CombinedPublicKey::Ed25519(pk) => {
                let pk_bytes = pk.to_bytes();
                let libp2p_pk = libp2p::core::PublicKey::Ed25519(
                    libp2p::core::identity::ed25519::PublicKey::decode(&pk_bytes)
                        .expect("Failed to decode public key"),
                );
                PeerId::from_public_key(&libp2p_pk)
            }
        }
    }
}

pub trait EnrForkId {
    fn fork_id(&self) -> Result<ForkId, &'static str>;
}

impl EnrForkId for Enr {
    fn fork_id(&self) -> Result<ForkId, &'static str> {
        let eth2_bytes = self.get(ETH2_ENR_KEY).ok_or("No eth2 enr key")?;
        ForkId::deserialize(eth2_bytes).map_err(|_| "Failed to decode fork id")
    }
}

// TODO: Do proper error handling
pub fn key_from_libp2p(key: &Keypair) -> Result<CombinedKey, Discv5Error> {
    match key {
        Keypair::Secp256k1(key) => {
            let secret = discv5::enr::k256::ecdsa::SigningKey::from_bytes(&key.secret().to_bytes())
                .map_err(|_| Discv5Error::KeyDerivationFailed)?;
            Ok(CombinedKey::Secp256k1(secret))
        }
        _ => Err(Discv5Error::KeyTypeNotSupported("The only supported key type is Secp256k1")),
    }
}
