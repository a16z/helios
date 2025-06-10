pub fn get_spec_id_for_block_timestamp(timestamp: u64, fork_schedule: &ForkSchedule) -> OpSpecId {
    if timestamp >= fork_schedule.isthmus_timestamp {
        OpSpecId::ISTHMUS
    } else if timestamp >= fork_schedule.holocene_timestamp {
        OpSpecId::HOLOCENE
    } else if timestamp >= fork_schedule.granite_timestamp {
        OpSpecId::GRANITE
    } else if timestamp >= fork_schedule.fjord_timestamp {
        OpSpecId::FJORD
    } else if timestamp >= fork_schedule.ecotone_timestamp {
        OpSpecId::ECOTONE
    } else if timestamp >= fork_schedule.canyon_timestamp {
        OpSpecId::CANYON
    } else if timestamp >= fork_schedule.regolith_timestamp {
        OpSpecId::REGOLITH
    } else if timestamp >= fork_schedule.bedrock_timestamp {
        OpSpecId::BEDROCK
    } else {
        OpSpecId::default()
    }

    if timestamp >= fork_schedule.prague_timestamp {
        SpecId::PRAGUE
    } else if timestamp >= fork_schedule.cancun_timestamp {
        SpecId::CANCUN
    } else if timestamp >= fork_schedule.shanghai_timestamp {
        SpecId::SHANGHAI
    } else if timestamp >= fork_schedule.paris_timestamp {
        SpecId::MERGE
    } else if timestamp >= fork_schedule.gray_glacier_timestamp {
        SpecId::GRAY_GLACIER
    } else if timestamp >= fork_schedule.arrow_glacier_timestamp {
        SpecId::ARROW_GLACIER
    } else if timestamp >= fork_schedule.london_timestamp {
        SpecId::LONDON
    } else if timestamp >= fork_schedule.berlin_timestamp {
        SpecId::BERLIN
    } else if timestamp >= fork_schedule.muir_glacier_timestamp {
        SpecId::MUIR_GLACIER
    } else if timestamp >= fork_schedule.istanbul_timestamp {
        SpecId::ISTANBUL
    } else if timestamp >= fork_schedule.petersburg_timestamp {
        SpecId::PETERSBURG
    } else if timestamp >= fork_schedule.constantinople_timestamp {
        SpecId::CONSTANTINOPLE
    } else if timestamp >= fork_schedule.byzantium_timestamp {
        SpecId::BYZANTIUM
    } else if timestamp >= fork_schedule.spurious_dragon_timestamp {
        SpecId::SPURIOUS_DRAGON
    } else if timestamp >= fork_schedule.tangerine_timestamp {
        SpecId::TANGERINE
    } else if timestamp >= fork_schedule.dao_timestamp {
        SpecId::DAO_FORK
    } else if timestamp >= fork_schedule.homestead_timestamp {
        SpecId::HOMESTEAD
    } else if timestamp >= fork_schedule.frontier_timestamp {
        SpecId::FRONTIER
    } else {
        SpecId::default()
    }
}
