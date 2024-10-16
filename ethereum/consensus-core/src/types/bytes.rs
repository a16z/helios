use serde::{Deserialize, Serialize};
use ssz_derive::{Decode, Encode};
use ssz_types::{
    serde_utils::{hex_fixed_vec, hex_var_list},
    FixedVector, VariableList,
};
use tree_hash_derive::TreeHash;

#[derive(Debug, Clone, Default, Encode, Decode, TreeHash, PartialEq)]
#[ssz(struct_behaviour = "transparent")]
pub struct ByteVector<N: typenum::Unsigned> {
    pub inner: FixedVector<u8, N>,
}

#[derive(Debug, Clone, Default, Encode, Decode, TreeHash, PartialEq)]
#[ssz(struct_behaviour = "transparent")]
pub struct ByteList<N: typenum::Unsigned> {
    pub inner: VariableList<u8, N>,
}

impl<'de, N: typenum::Unsigned> serde::Deserialize<'de> for ByteVector<N> {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let inner = hex_fixed_vec::deserialize(deserializer)?;
        Ok(Self { inner })
    }
}

impl<N: typenum::Unsigned> Serialize for ByteVector<N> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        hex_fixed_vec::serialize(&self.inner, serializer)
    }
}

impl<'de, N: typenum::Unsigned> Deserialize<'de> for ByteList<N> {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let inner = hex_var_list::deserialize(deserializer)?;
        Ok(Self { inner })
    }
}

impl<N: typenum::Unsigned> serde::Serialize for ByteList<N> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        hex_var_list::serialize(&self.inner, serializer)
    }
}
