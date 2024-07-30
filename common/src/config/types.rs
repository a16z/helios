use serde::{Deserialize, Serialize};

use crate::utils::{bytes_deserialize, bytes_serialize};

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Forks {
    pub genesis: Fork,
    pub altair: Fork,
    pub bellatrix: Fork,
    pub capella: Fork,
    pub deneb: Fork,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Fork {
    pub epoch: u64,
    #[serde(
        deserialize_with = "bytes_deserialize",
        serialize_with = "bytes_serialize"
    )]
    pub fork_version: Vec<u8>,
}
