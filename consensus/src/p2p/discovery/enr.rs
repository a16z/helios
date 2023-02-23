use discv5::{
    enr::{self, CombinedKey},
    Enr, Discv5Error,
};
use libp2p::identity::Keypair;
use crate::p2p::{ 
    config::Config,
    utils::ForkId,
};
use ssz_rs::Serialize;

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
    &ForkId::new().serialize(&mut bytes).unwrap();
    enr_builder.add_value(ETH2_ENR_KEY, bytes.as_slice());

    enr_builder.build(key).unwrap()
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
