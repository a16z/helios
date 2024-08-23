pub mod u64 {
    use serde::{Serializer, Deserializer, de::Error};

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
        u64::from_str_radix(&val, 10).map_err(D::Error::custom)
    }
}

pub mod u256 {
    use alloy::primitives::U256;
    use serde::{Deserializer, de::Error};
    
    pub fn deserialize<'de, D>(deserializer: D) -> Result<U256, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val: String = serde::Deserialize::deserialize(deserializer)?;
        U256::from_str_radix(&val, 10).map_err(D::Error::custom)
    }
}

pub mod header {
    use serde::{Deserialize, Deserializer};
    use crate::types::Header;

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
