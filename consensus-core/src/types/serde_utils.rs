pub mod u64 {
    use alloc::string::{String, ToString};
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

pub mod u256 {
    use alloc::string::String;
    use alloy_primitives::U256;
    use serde::{de::Error, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<U256, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val: String = serde::Deserialize::deserialize(deserializer)?;
        val.parse().map_err(D::Error::custom)
    }
}

pub mod header {
    use crate::types::Header;
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Header, D::Error>
    where
        D: Deserializer<'de>,
    {
        let header: LightClientHeader = Deserialize::deserialize(deserializer)?;

        Ok(match header {
            LightClientHeader::Unwrapped(header) => header,
            LightClientHeader::Wrapped(header) => header.beacon,
        })
    }

    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum LightClientHeader {
        Unwrapped(Header),
        Wrapped(Beacon),
    }

    #[derive(serde::Deserialize)]
    struct Beacon {
        beacon: Header,
    }
}
