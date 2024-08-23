use ssz_types::{FixedVector, VariableList};
use ssz_derive::{Encode, Decode};
use tree_hash_derive::TreeHash;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Default, Encode, Decode, TreeHash)]
#[ssz(struct_behaviour = "transparent")]
pub struct ByteVector<N: typenum::Unsigned> {
    pub inner: FixedVector<u8, N>
}

#[derive(Debug, Clone, Default, Encode, Decode, TreeHash)]
#[ssz(struct_behaviour = "transparent")]
pub struct ByteList<N: typenum::Unsigned> {
    pub inner: VariableList<u8, N>
}

impl<'de, N: typenum::Unsigned> serde::Deserialize<'de> for ByteVector<N> {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bytes: String = serde::Deserialize::deserialize(deserializer)?;
        let bytes = hex::decode(bytes.strip_prefix("0x").unwrap()).unwrap();
        Ok(Self {
            inner: bytes.to_vec().try_into().unwrap(),
        })
    }
}

impl<N: typenum::Unsigned> Serialize for ByteVector<N> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let hex_string = format!("0x{}", hex::encode(self.inner.to_vec()));
        serializer.serialize_str(&hex_string)
    }
}

impl<'de, N: typenum::Unsigned> Deserialize<'de> for ByteList<N> {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bytes: String = Deserialize::deserialize(deserializer)?;
        let bytes = hex::decode(bytes.strip_prefix("0x").unwrap()).unwrap();
        Ok(Self {
            inner: bytes.to_vec().try_into().unwrap(),
        })
    }
}

impl<N: typenum::Unsigned> serde::Serialize for ByteList<N> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let hex_string = format!("0x{}", hex::encode(self.inner.to_vec()));
        serializer.serialize_str(&hex_string)
    }
}

