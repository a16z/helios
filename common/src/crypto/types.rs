use crate::{
    consensus::types::{BLSPubKey, SignatureBytes},
    crypto::consts::CURVE_ORDER,
};
use eyre::Result;

#[cfg(feature = "bls12_381")]
mod bls12_381_impl {
    use super::*;
    use bls12_381::{multi_miller_loop, G1Affine, G1Projective, G2Affine, G2Prepared, Gt, Scalar};

    /// A BLS public key.
    #[derive(Debug)]
    pub struct PublicKey {
        pub point: G1Affine,
    }

    impl PublicKey {
        /// Instantiate a PublicKey from compressed bytes.
        pub fn from_bytes_unchecked(bytes: &BLSPubKey) -> Result<Self> {
            let point: G1Affine =
                G1Affine::from_compressed(bytes.as_ref().try_into().unwrap()).unwrap();
            Ok(Self { point })
        }

        /// Instantiate a new aggregate public key from a vector of PublicKeys.
        pub fn aggregate(keys: &[&PublicKey]) -> Result<Self> {
            if keys.is_empty() {
                return Err(eyre::eyre!("No keys to aggregate"));
            }

            let mut agg_key = G1Projective::identity();
            for key in keys {
                agg_key += G1Projective::from(key.point);
            }
            Ok(Self {
                point: G1Affine::from(agg_key),
            })
        }
    }

    /// Allows for the adding/combining of multiple BLS Signatures.
    ///
    /// This may be verified against some aggregated public key.
    #[derive(Debug)]
    pub struct AggregateSignature {
        pub point: G2Affine,
    }

    impl AggregateSignature {
        /// Instatiate an AggregateSignature from some bytes.
        pub fn from_bytes(bytes: &SignatureBytes) -> Result<Self> {
            let point = G2Affine::from_compressed(bytes.as_ref().try_into().unwrap()).unwrap();
            Ok(Self { point })
        }

        /// FastAggregateVerify
        ///
        /// Verifies an AggregateSignature against a list of PublicKeys.
        /// PublicKeys must all be verified via Proof of Possession before running this function.
        /// https://tools.ietf.org/html/draft-irtf-cfrg-bls-signature-02#section-3.3.4
        pub fn fast_aggregate_verify(&self, msg: &[u8], public_keys: &[&PublicKey]) -> bool {
            println!("cycle-tracker-start: fast_aggregate_verify");
            // Require at least one PublicKey
            if public_keys.is_empty() {
                return false;
            }

            println!("cycle-tracker-start: subgroup-check-g2");
            // Subgroup check for signature
            if !subgroup_check_g2(&self.point) {
                return false;
            }
            println!("cycle-tracker-end: subgroup-check-g2");

            // Aggregate PublicKeys
            println!("cycle-tracker-start: aggregate-public-keys");
            let aggregate_public_key = PublicKey::aggregate(public_keys);
            println!("cycle-tracker-end: aggregate-public-keys");
            if aggregate_public_key.is_err() {
                return false;
            }
            let aggregate_public_key = aggregate_public_key.unwrap();

            // Ensure AggregatePublicKey is not infinity
            if aggregate_public_key.point.is_identity().into() {
                return false;
            }

            // Points must be affine for pairing
            let sig_point = self.point;
            let key_point = aggregate_public_key.point;
            println!("cycle-tracker-start: hash-to-curve-g2");
            let msg_hash = G2Affine::from(bls12_381::G2Projective::hash_to_curve_g2(msg));
            println!("cycle-tracker-end: hash-to-curve-g2");
            let generator_g1_negative = G1Affine::from(-G1Projective::generator());

            // Faster ate2 evaualtion checks e(S, -G1) * e(H, PK) == 1
            println!("cycle-tracker-start: ate2-evaluation");
            let temp = ate2_evaluation(&sig_point, &generator_g1_negative, &msg_hash, &key_point);
            println!("cycle-tracker-end: ate2-evaluation");

            println!("cycle-tracker-end: fast_aggregate_verify");
            temp
        }
    }

    // Verifies a G2 point is in subgroup `r`.
    pub fn subgroup_check_g2(point: &G2Affine) -> bool {
        let r = Scalar::from_hex(CURVE_ORDER).unwrap();
        let check = point * r;
        check.is_identity().into()
    }

    /// Evaluation of e(S, -G1) * e(H, PK) == 1
    pub fn ate2_evaluation(p1: &G2Affine, q1: &G1Affine, r1: &G2Affine, s1: &G1Affine) -> bool {
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
}

#[cfg(feature = "bls12_381")]
pub use bls12_381_impl::*;
