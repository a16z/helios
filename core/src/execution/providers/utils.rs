use alloy::rpc::types::{Filter, Log};
use eyre::Result;

use crate::execution::errors::ExecutionError;

pub fn ensure_logs_match_filter(logs: &[Log], filter: &Filter) -> Result<()> {
    fn log_matches_filter(log: &Log, filter: &Filter) -> bool {
        if let Some(block_hash) = filter.get_block_hash() {
            if log.block_hash.unwrap() != block_hash {
                return false;
            }
        }
        if let Some(from_block) = filter.get_from_block() {
            if log.block_number.unwrap() < from_block {
                return false;
            }
        }
        if let Some(to_block) = filter.get_to_block() {
            if log.block_number.unwrap() > to_block {
                return false;
            }
        }
        if !filter.address.matches(&log.address()) {
            return false;
        }
        for (i, filter_topic) in filter.topics.iter().enumerate() {
            if !filter_topic.is_empty() {
                if let Some(log_topic) = log.topics().get(i) {
                    if !filter_topic.matches(log_topic) {
                        return false;
                    }
                } else {
                    // if filter topic is not present in log, it's a mismatch
                    return false;
                }
            }
        }
        true
    }

    for log in logs {
        if !log_matches_filter(log, filter) {
            return Err(ExecutionError::LogFilterMismatch().into());
        }
    }

    Ok(())
}
