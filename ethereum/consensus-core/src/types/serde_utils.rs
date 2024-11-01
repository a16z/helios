pub mod u64 {
    use serde::{de::Error, Deserializer, Serializer};

    pub fn serialize<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val: String = serde::Deserialize::deserialize(deserializer)?;
        val.parse().map_err(D::Error::custom)
    }
}

pub mod u64_opt {
    use serde::{de::Error, Deserializer, Serializer};

    pub fn serialize<S>(value: &Option<u64>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if let Some(value) = value {
            serializer.serialize_str(&value.to_string())
        } else {
            serializer.serialize_none()
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val: Option<String> = serde::Deserialize::deserialize(deserializer)?;
        if let Some(n) = val {
            Ok(Some(n.parse().map_err(D::Error::custom)?))
        } else {
            Ok(None)
        }
    }
}

pub mod u8_opt {
    use serde::{de::Error, Deserializer, Serializer};

    pub fn serialize<S>(value: &Option<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if let Some(value) = value {
            serializer.serialize_str(&value.to_string())
        } else {
            serializer.serialize_none()
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val: Option<String> = serde::Deserialize::deserialize(deserializer)?;
        if let Some(n) = val {
            Ok(Some(n.parse().map_err(D::Error::custom)?))
        } else {
            Ok(None)
        }
    }
}

pub mod u256 {
    use alloy::primitives::U256;
    use serde::{de::Error, Deserializer, Serializer};

    pub fn serialize<S>(value: &U256, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<U256, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val: String = serde::Deserialize::deserialize(deserializer)?;
        val.parse().map_err(D::Error::custom)
    }
}

pub mod u256_opt {
    use alloy::primitives::U256;
    use serde::{de::Error, Deserializer, Serializer};

    pub fn serialize<S>(value: &Option<U256>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if let Some(value) = value {
            serializer.serialize_str(&value.to_string())
        } else {
            serializer.serialize_none()
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<U256>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val: Option<String> = serde::Deserialize::deserialize(deserializer)?;
        if let Some(n) = val {
            Ok(Some(n.parse().map_err(D::Error::custom)?))
        } else {
            Ok(None)
        }
    }
}
