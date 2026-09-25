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
    pub osaka_timestamp: u64,
    #[serde(default = "inactive_fork")]
    pub bpo1_timestamp: u64,
    #[serde(default = "inactive_fork")]
    pub bpo2_timestamp: u64,

    // Optimism Forks
    pub bedrock_timestamp: u64,
    pub regolith_timestamp: u64,
    pub canyon_timestamp: u64,
    pub delta_timestamp: u64,
    pub ecotone_timestamp: u64,
    pub fjord_timestamp: u64,
    pub granite_timestamp: u64,
    pub holocene_timestamp: u64,
    pub isthmus_timestamp: u64,
    pub jovian_timestamp: u64,
}

impl Default for ForkSchedule {
    fn default() -> Self {
        ForkSchedule {
            // u64::MAX represents a fork timestamp that is effectively "not set" or "in the future" or "not activated yet"
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
            osaka_timestamp: u64::MAX,
            bpo1_timestamp: u64::MAX,
            bpo2_timestamp: u64::MAX,

            bedrock_timestamp: u64::MAX,
            regolith_timestamp: u64::MAX,
            canyon_timestamp: u64::MAX,
            delta_timestamp: u64::MAX,
            ecotone_timestamp: u64::MAX,
            fjord_timestamp: u64::MAX,
            granite_timestamp: u64::MAX,
            holocene_timestamp: u64::MAX,
            isthmus_timestamp: u64::MAX,
            jovian_timestamp: u64::MAX,
        }
    }
}

impl ForkSchedule {
    /// Get the blob base fee update fraction for a given timestamp.
    /// EIP-7691 changes the Prague fraction; EIP-7892 permits later BPO changes.
    pub fn get_blob_base_fee_update_fraction(&self, timestamp: u64) -> u64 {
        if self.bpo2_timestamp != u64::MAX && timestamp >= self.bpo2_timestamp {
            11684671
        } else if self.bpo1_timestamp != u64::MAX && timestamp >= self.bpo1_timestamp {
            8346193
        } else if self.prague_timestamp != u64::MAX && timestamp >= self.prague_timestamp {
            5007716
        } else {
            3338477 // Cancun
        }
    }
}

fn inactive_fork() -> u64 {
    u64::MAX
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blob_prices_follow_each_activation_boundary() {
        let forks = ForkSchedule {
            prague_timestamp: 10,
            osaka_timestamp: 20,
            bpo1_timestamp: 30,
            bpo2_timestamp: 40,
            ..Default::default()
        };
        for (timestamp, fraction) in [
            (9, 3338477),
            (10, 5007716),
            (19, 5007716),
            (20, 5007716),
            (29, 5007716),
            (30, 8346193),
            (39, 8346193),
            (40, 11684671),
        ] {
            assert_eq!(forks.get_blob_base_fee_update_fraction(timestamp), fraction);
        }
    }

    #[test]
    fn older_configurations_leave_bpo_forks_inactive() {
        let mut json = serde_json::to_value(ForkSchedule::default()).unwrap();
        let fields = json.as_object_mut().unwrap();
        fields.remove("bpo1_timestamp");
        fields.remove("bpo2_timestamp");
        let forks: ForkSchedule = serde_json::from_value(json).unwrap();
        assert_eq!(forks.bpo1_timestamp, u64::MAX);
        assert_eq!(forks.bpo2_timestamp, u64::MAX);
        assert_eq!(forks.get_blob_base_fee_update_fraction(u64::MAX), 3338477);
    }
}
