use alloc::vec::Vec;
use anyhow::{anyhow, Result};
use bls12_381::{
    hash_to_curve::{ExpandMsgXmd, HashToCurve},
    multi_miller_loop, G1Affine, G1Projective, G2Affine, G2Prepared, G2Projective, Gt, Scalar,
};
use serde::{Deserialize, Serialize};
use ssz_derive::Encode;
use tree_hash_derive::TreeHash;

use super::bytes::ByteVector;

#[derive(Debug, Clone, Default, Serialize, Deserialize, Encode, TreeHash)]
#[ssz(struct_behaviour = "transparent")]
#[serde(transparent)]
pub struct PublicKey {
    inner: ByteVector<typenum::U48>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Encode, TreeHash)]
#[ssz(struct_behaviour = "transparent")]
#[serde(transparent)]
pub struct Signature {
    inner: ByteVector<typenum::U96>,
}

impl PublicKey {
    fn point(&self) -> Result<G1Affine> {
        let bytes = self.inner.inner.to_vec();
        let bytes = bytes
            .as_slice()
            .try_into()
            .map_err(|_| anyhow!("Invalid byte length"))?;
        let point_opt = G1Affine::from_compressed(bytes);
        if point_opt.is_some().into() {
            Ok(point_opt.unwrap())
        } else {
            Err(anyhow!("invalid point"))
        }
    }
}

impl Signature {
    /// FastAggregateVerify
    ///
    /// Verifies an AggregateSignature against a list of PublicKeys.
    /// PublicKeys must all be verified via Proof of Possession before running this function.
    /// https://tools.ietf.org/html/draft-irtf-cfrg-bls-signature-02#section-3.3.4
    pub fn verify(&self, msg: &[u8], pks: &[PublicKey]) -> bool {
        let sig_point = if let Ok(point) = self.point() {
            point
        } else {
            return false;
        };

        // Subgroup check for signature
        if !subgroup_check_g2(&sig_point) {
            return false;
        }

        // Aggregate PublicKeys
        let aggregate_public_key = if let Ok(agg) = aggregate(pks) {
            agg
        } else {
            return false;
        };

        // Ensure AggregatePublicKey is not infinity
        if aggregate_public_key.is_identity().into() {
            return false;
        }

        // Points must be affine for pairing
        let key_point = aggregate_public_key;
        let msg_hash = G2Affine::from(hash_to_curve(msg));

        let generator_g1_negative = G1Affine::from(-G1Projective::generator());

        // Faster ate2 evaualtion checks e(S, -G1) * e(H, PK) == 1
        ate2_evaluation(&sig_point, &generator_g1_negative, &msg_hash, &key_point)
    }

    fn point(&self) -> Result<G2Affine> {
        let bytes = self.inner.inner.to_vec();
        let bytes = bytes
            .as_slice()
            .try_into()
            .map_err(|_| anyhow!("Invalid byte length"))?;

        let point_opt = G2Affine::from_compressed(bytes);
        if point_opt.is_some().into() {
            Ok(point_opt.unwrap())
        } else {
            Err(anyhow!("invalid point"))
        }
    }
}

/// Aggregates multiple keys into one aggragate key
fn aggregate(pks: &[PublicKey]) -> Result<G1Affine> {
    if pks.is_empty() {
        return Err(anyhow!("no keys to aggregate"));
    }

    let mut agg_key = G1Projective::identity();
    for key in pks {
        agg_key += G1Projective::from(key.point()?)
    }

    Ok(G1Affine::from(agg_key))
}

/// Verifies a G2 point is in subgroup `r`.
fn subgroup_check_g2(point: &G2Affine) -> bool {
    const CURVE_ORDER: &str = "73EDA753299D7D483339D80809A1D80553BDA402FFFE5BFEFFFFFFFF00000001";
    let r = hex_to_scalar(CURVE_ORDER).unwrap();
    let check = point * r;
    check.is_identity().into()
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

/// Converts hex string to scalar
fn hex_to_scalar(hex: &str) -> Option<Scalar> {
    if hex.len() != 64 {
        return None;
    }

    let mut raw = [0u64; 4];
    for (i, chunk) in hex.as_bytes().chunks(16).enumerate().take(4) {
        if let Ok(hex_chunk) = core::str::from_utf8(chunk) {
            if let Ok(value) = u64::from_str_radix(hex_chunk, 16) {
                raw[3 - i] = value.to_le();
            } else {
                return None;
            }
        } else {
            return None;
        }
    }

    Some(Scalar::from_raw(raw))
}
