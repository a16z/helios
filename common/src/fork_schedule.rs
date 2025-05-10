use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub struct ForkSchedule {
    // Ethereum Forks
    pub frontier_timestamp: u64,
    pub homestead_timestamp: u64,
    pub dao_timestamp: u64,
    pub tangerine_timestamp: u64,
    pub spurious_dragon_timestamp: u64,
    pub byzantium_timestamp: u64,
    pub constantinople_timestamp: u64, // Overridden by Petersburg
    pub petersburg_timestamp: u64,
    pub istanbul_timestamp: u64,
    pub muir_glacier_timestamp: u64,
    pub berlin_timestamp: u64,
    pub london_timestamp: u64,
    pub arrow_glacier_timestamp: u64,
    pub gray_glacier_timestamp: u64,
    pub paris_timestamp: u64, // Represents Merge
    pub shanghai_timestamp: u64,
    pub cancun_timestamp: u64,
    pub prague_timestamp: u64,
    // Osaka and beyond TBD

    // Optimism Forks
    pub bedrock_timestamp: u64,
    pub regolith_timestamp: u64,
    pub canyon_timestamp: u64,
    pub ecotone_timestamp: u64,
    pub fjord_timestamp: u64,
    pub granite_timestamp: u64,
    pub holocene_timestamp: u64,
    pub isthmus_timestamp: u64,
}

impl Default for ForkSchedule {
    fn default() -> Self {
        ForkSchedule {
            frontier_timestamp: u64::MAX,
            homestead_timestamp: u64::MAX,
            dao_timestamp: u64::MAX,
            tangerine_timestamp: u64::MAX,
            spurious_dragon_timestamp: u64::MAX,
            byzantium_timestamp: u64::MAX,
            constantinople_timestamp: u64::MAX,
            petersburg_timestamp: u64::MAX,
            istanbul_timestamp: u64::MAX,
            muir_glacier_timestamp: u64::MAX,
            berlin_timestamp: u64::MAX,
            london_timestamp: u64::MAX,
            arrow_glacier_timestamp: u64::MAX,
            gray_glacier_timestamp: u64::MAX,
            paris_timestamp: u64::MAX,
            shanghai_timestamp: u64::MAX,
            cancun_timestamp: u64::MAX,
            prague_timestamp: u64::MAX,

            bedrock_timestamp: u64::MAX,
            regolith_timestamp: u64::MAX,
            canyon_timestamp: u64::MAX,
            ecotone_timestamp: u64::MAX,
            fjord_timestamp: u64::MAX,
            granite_timestamp: u64::MAX,
            holocene_timestamp: u64::MAX,
            isthmus_timestamp: u64::MAX,
        }
    }
}
