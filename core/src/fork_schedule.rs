use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize, Default, Debug)]
pub struct ForkSchedule {
    pub prague_timestamp: u64,
}
