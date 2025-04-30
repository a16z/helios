use helios_common::fork_schedule::ForkSchedule;
use revm::primitives::SpecId;

use crate::types::RevmNetwork;

pub fn get_spec_id_for_block(
    network: &RevmNetwork,
    fork_schedule: &ForkSchedule,
    block_timestamp: u64,
) -> Option<SpecId> {
    match network {
        RevmNetwork::Ethereum => {
            if block_timestamp >= fork_schedule.prague_timestamp {
                Some(SpecId::PRAGUE)
            } else if block_timestamp >= fork_schedule.cancun_timestamp {
                Some(SpecId::CANCUN)
            } else if block_timestamp >= fork_schedule.shanghai_timestamp {
                Some(SpecId::SHANGHAI)
            } else if block_timestamp >= fork_schedule.paris_timestamp {
                Some(SpecId::MERGE)
            } else if block_timestamp >= fork_schedule.arrow_glacier_timestamp {
                Some(SpecId::ARROW_GLACIER)
            } else if block_timestamp >= fork_schedule.gray_glacier_timestamp {
                Some(SpecId::GRAY_GLACIER)
            } else if block_timestamp >= fork_schedule.london_timestamp {
                Some(SpecId::LONDON)
            } else if block_timestamp >= fork_schedule.berlin_timestamp {
                Some(SpecId::BERLIN)
            } else if block_timestamp >= fork_schedule.muir_glacier_timestamp {
                Some(SpecId::MUIR_GLACIER)
            } else if block_timestamp >= fork_schedule.istanbul_timestamp {
                Some(SpecId::ISTANBUL)
            } else if block_timestamp >= fork_schedule.petersburg_timestamp {
                Some(SpecId::PETERSBURG)
            } else if block_timestamp >= fork_schedule.byzantium_timestamp {
                Some(SpecId::BYZANTIUM)
            } else if block_timestamp >= fork_schedule.spurious_dragon_timestamp {
                Some(SpecId::SPURIOUS_DRAGON)
            } else if block_timestamp >= fork_schedule.tangerine_timestamp {
                Some(SpecId::TANGERINE)
            } else if block_timestamp >= fork_schedule.dao_timestamp {
                Some(SpecId::DAO_FORK)
            } else if block_timestamp >= fork_schedule.homestead_timestamp {
                Some(SpecId::HOMESTEAD)
            } else if block_timestamp >= fork_schedule.frontier_timestamp {
                Some(SpecId::FRONTIER)
            } else {
                Some(SpecId::LATEST)
            }
        }
        RevmNetwork::OpStack => {
            if block_timestamp >= fork_schedule.holocene_timestamp {
                Some(SpecId::HOLOCENE)
            } else if block_timestamp >= fork_schedule.granite_timestamp {
                Some(SpecId::GRANITE)
            } else if block_timestamp >= fork_schedule.fjord_timestamp {
                Some(SpecId::FJORD)
            } else if block_timestamp >= fork_schedule.ecotone_timestamp {
                Some(SpecId::ECOTONE)
            } else if block_timestamp >= fork_schedule.canyon_timestamp {
                Some(SpecId::CANYON)
            } else if block_timestamp >= fork_schedule.regolith_timestamp {
                Some(SpecId::REGOLITH)
            } else if block_timestamp >= fork_schedule.bedrock_timestamp {
                Some(SpecId::BEDROCK)
            } else {
                Some(SpecId::LATEST)
            }
        }
        RevmNetwork::Linea => Some(SpecId::LONDON),
    }
}
