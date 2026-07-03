use bls12_381::{
    hash_to_curve::{ExpandMsgXmd, HashToCurve},
    multi_miller_loop, G1Affine, G1Projective, G2Affine, G2Prepared, G2Projective, Gt,
};
use eyre::{eyre, Result};
use serde::{Deserialize, Serialize};
use ssz_derive::{Decode, Encode};
use tree_hash_derive::TreeHash;

use super::bytes::ByteVector;

#[derive(Debug, Clone, Default, Serialize, Deserialize, Encode, Decode, TreeHash, PartialEq)]
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

impl PublicKey {
    pub(crate) fn point(&self) -> Result<G1Affine> {
        let bytes = self.inner.inner.to_vec();
        let bytes = bytes.as_slice().try_into()?;
        let point_opt = G1Affine::from_compressed(bytes);
        if point_opt.is_some().into() {
            Ok(point_opt.unwrap())
        } else {
            Err(eyre!("invalid point"))
        }
    }
}

impl Signature {
    pub fn verify(&self, msg: &[u8], aggregate_public_key: &G1Affine) -> bool {
        let sig_point = if let Ok(point) = self.point() {
            point
        } else {
            return false;
        };

        verify_with_aggregate_pk(&sig_point, msg, aggregate_public_key)
    }

    fn point(&self) -> Result<G2Affine> {
        let bytes = self.inner.inner.to_vec();
        let bytes = bytes.as_slice().try_into()?;
        let point_opt = G2Affine::from_compressed(bytes);
        if point_opt.is_some().into() {
            Ok(point_opt.unwrap())
        } else {
            Err(eyre!("invalid point"))
        }
    }
}

fn verify_with_aggregate_pk(
    sig_point: &G2Affine,
    msg: &[u8],
    aggregate_public_key: &G1Affine,
) -> bool {
    // Ensure AggregatePublicKey is not infinity
    if aggregate_public_key.is_identity().into() {
        return false;
    }

    // Points must be affine for pairing
    let key_point = *aggregate_public_key;
    let msg_hash = G2Affine::from(hash_to_curve(msg));

    let generator_g1_negative = G1Affine::from(-G1Projective::generator());

    // Faster ate2 evaluation checks e(S, -G1) * e(H, PK) == 1
    ate2_evaluation(sig_point, &generator_g1_negative, &msg_hash, &key_point)
}

/// Evaluation of e(S, -G1) * e(H, PK) == 1
fn ate2_evaluation(p1: &G2Affine, q1: &G1Affine, r1: &G2Affine, s1: &G1Affine) -> bool {
    // Prepare G2 points for efficient pairing
    let signature_prepared = G2Prepared::from(*p1);
    let msg_hash_prepared = G2Prepared::from(*r1);

    // Compute e(S, -G1) * e(H, PK)
    let pairing = multi_miller_loop(&[(q1, &signature_prepared), (s1, &msg_hash_prepared)]);

    // Perform final exponentiation
    let result = pairing.final_exponentiation();

    // Check if the result is equal to the identity element of Gt
    result == Gt::identity()
}

/// Hash a message to the curve
fn hash_to_curve(msg: &[u8]) -> G2Projective {
    const DST: &[u8] = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_";
    <G2Projective as HashToCurve<ExpandMsgXmd<sha2::Sha256>>>::hash_to_curve(msg, DST)
}
