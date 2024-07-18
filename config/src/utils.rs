use common::utils::hex_str_to_bytes;

pub fn bytes_opt_deserialize<'de, D>(deserializer: D) -> Result<Option<Vec<u8>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let bytes_opt: Option<String> = serde::Deserialize::deserialize(deserializer)?;
    if let Some(bytes) = bytes_opt {
        Ok(Some(hex_str_to_bytes(&bytes).unwrap()))
    } else {
        Ok(None)
    }
}
