use milagro_bls::{AggregateSignature, PublicKey as MilagroPK};
use serde::{Deserialize, Serialize};
use ssz_derive::{Decode, Encode};
use tree_hash_derive::TreeHash;

use super::bytes::ByteVector;

#[derive(Debug, Clone, Default, Serialize, Deserialize, Encode, Decode, TreeHash)]
#[ssz(struct_behaviour = "transparent")]
#[serde(transparent)]
pub struct PublicKey {
    inner: ByteVector<typenum::U48>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Encode, Decode, TreeHash)]
#[ssz(struct_behaviour = "transparent")]
#[serde(transparent)]
pub struct Signature {
    inner: ByteVector<typenum::U96>,
}

impl Signature {
    pub fn verify(&self, msg: &[u8], pks: &[PublicKey]) -> bool {
        if let Ok(agg) = AggregateSignature::from_bytes(&self.inner.inner) {
            let pks_res = pks
                .iter()
                .map(|pk| MilagroPK::from_bytes(&pk.inner.inner))
                .collect::<Result<Vec<_>, _>>();

            if let Ok(pks) = pks_res {
                let pks = pks.iter().collect::<Vec<_>>();
                agg.fast_aggregate_verify(msg, &pks)
            } else {
                false
            }
        } else {
            false
        }
    }
}
