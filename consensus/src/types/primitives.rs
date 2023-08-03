use std::ops::Deref;

use ssz_rs::prelude::*;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ByteVector<const N: usize> {
    inner: Vector<u8, N>,
}

impl<const N: usize> ByteVector<N> {
    pub fn as_slice(&self) -> &[u8] {
        self.inner.as_slice()
    }
}

impl<const N: usize> Deref for ByteVector<N> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.inner.as_slice()
    }
}

impl<const N: usize> TryFrom<Vec<u8>> for ByteVector<N> {
    type Error = eyre::Report;

    fn try_from(value: Vec<u8>) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            inner: Vector::try_from(value).map_err(|(_, err)| err)?,
        })
    }
}

impl<const N: usize> TryFrom<&[u8]> for ByteVector<N> {
    type Error = eyre::Report;

    fn try_from(value: &[u8]) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            inner: Vector::try_from(value.to_vec()).map_err(|(_, err)| err)?,
        })
    }
}

impl<const N: usize> ssz_rs::Merkleized for ByteVector<N> {
    fn hash_tree_root(&mut self) -> std::result::Result<Node, MerkleizationError> {
        self.inner.hash_tree_root()
    }
}

impl<const N: usize> ssz_rs::Sized for ByteVector<N> {
    fn size_hint() -> usize {
        0
    }

    fn is_variable_size() -> bool {
        false
    }
}

impl<const N: usize> ssz_rs::Serialize for ByteVector<N> {
    fn serialize(&self, buffer: &mut Vec<u8>) -> std::result::Result<usize, SerializeError> {
        self.inner.serialize(buffer)
    }
}

impl<const N: usize> ssz_rs::Deserialize for ByteVector<N> {
    fn deserialize(encoding: &[u8]) -> std::result::Result<Self, DeserializeError>
    where
        Self: std::marker::Sized,
    {
        Ok(Self {
            inner: Vector::deserialize(encoding)?,
        })
    }
}

impl<const N: usize> ssz_rs::SimpleSerialize for ByteVector<N> {}

impl<'de, const N: usize> serde::Deserialize<'de> for ByteVector<N> {
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ByteList<const N: usize> {
    inner: List<u8, N>,
}

impl<const N: usize> ByteList<N> {
    pub fn as_slice(&self) -> &[u8] {
        self.inner.as_slice()
    }
}

impl<const N: usize> Deref for ByteList<N> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.inner.as_slice()
    }
}

impl<const N: usize> TryFrom<Vec<u8>> for ByteList<N> {
    type Error = eyre::Report;

    fn try_from(value: Vec<u8>) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            inner: List::try_from(value).map_err(|(_, err)| err)?,
        })
    }
}

impl<const N: usize> TryFrom<&[u8]> for ByteList<N> {
    type Error = eyre::Report;

    fn try_from(value: &[u8]) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            inner: List::try_from(value.to_vec()).map_err(|(_, err)| err)?,
        })
    }
}

impl<const N: usize> ssz_rs::Merkleized for ByteList<N> {
    fn hash_tree_root(&mut self) -> std::result::Result<Node, MerkleizationError> {
        self.inner.hash_tree_root()
    }
}

impl<const N: usize> ssz_rs::Sized for ByteList<N> {
    fn size_hint() -> usize {
        0
    }

    fn is_variable_size() -> bool {
        false
    }
}

impl<const N: usize> ssz_rs::Serialize for ByteList<N> {
    fn serialize(&self, buffer: &mut Vec<u8>) -> std::result::Result<usize, SerializeError> {
        self.inner.serialize(buffer)
    }
}

impl<const N: usize> ssz_rs::Deserialize for ByteList<N> {
    fn deserialize(encoding: &[u8]) -> std::result::Result<Self, DeserializeError>
    where
        Self: std::marker::Sized,
    {
        Ok(Self {
            inner: List::deserialize(encoding)?,
        })
    }
}

impl<const N: usize> ssz_rs::SimpleSerialize for ByteList<N> {}

impl<'de, const N: usize> serde::Deserialize<'de> for ByteList<N> {
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

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct U64 {
    inner: u64,
}

impl U64 {
    pub fn as_u64(&self) -> u64 {
        self.inner
    }
}

impl From<U64> for u64 {
    fn from(value: U64) -> Self {
        value.inner
    }
}

impl ssz_rs::Merkleized for U64 {
    fn hash_tree_root(&mut self) -> std::result::Result<Node, MerkleizationError> {
        self.inner.hash_tree_root()
    }
}

impl ssz_rs::Sized for U64 {
    fn size_hint() -> usize {
        0
    }

    fn is_variable_size() -> bool {
        false
    }
}

impl ssz_rs::Serialize for U64 {
    fn serialize(&self, buffer: &mut Vec<u8>) -> std::result::Result<usize, SerializeError> {
        self.inner.serialize(buffer)
    }
}

impl ssz_rs::Deserialize for U64 {
    fn deserialize(encoding: &[u8]) -> std::result::Result<Self, DeserializeError>
    where
        Self: std::marker::Sized,
    {
        Ok(Self {
            inner: u64::deserialize(encoding)?,
        })
    }
}

impl ssz_rs::SimpleSerialize for U64 {}

impl<'de> serde::Deserialize<'de> for U64 {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let val: String = serde::Deserialize::deserialize(deserializer)?;
        Ok(Self {
            inner: val.parse().unwrap(),
        })
    }
}
