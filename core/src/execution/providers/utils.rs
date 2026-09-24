use crate::execution::constants::MAX_SUPPORTED_BLOCKS_TO_PROVE_FOR_LOGS;
use alloy::rpc::types::{BlockId, BlockNumberOrTag, Filter, FilterBlockOption, Log};
use alloy::{
    consensus::BlockHeader,
    network::{primitives::HeaderResponse, BlockResponse},
};
use eyre::{eyre, Result};
use helios_common::{
    execution_provider::{BlockProvider, ReceiptProvider},
    network_spec::NetworkSpec,
};

use crate::execution::errors::ExecutionError;

/// Determines if a block ID should be fetched from the historical provider.
/// Historical providers should only be used for specific block numbers or hashes,
/// never for block tags like Latest, Safe, Finalized, etc.
pub fn should_use_historical_provider(block_id: &BlockId) -> bool {
    match block_id {
        BlockId::Number(BlockNumberOrTag::Number(_)) => true,
        BlockId::Hash(_) => true,
        _ => false, // Don't use for Latest, Safe, Finalized, Pending, Earliest
    }
}

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

pub async fn get_verified_logs<N: NetworkSpec, E: BlockProvider<N> + ReceiptProvider<N>>(
    execution: &E,
    filter: &Filter,
) -> Result<Vec<Log>> {
    let (mut block, from) = match filter.block_option {
        FilterBlockOption::AtBlockHash(hash) => {
            let block = execution
                .get_block(hash.into(), false)
                .await?
                .ok_or(eyre!("block not found"))?;
            let number = block.header().number();
            (block, number)
        }
        FilterBlockOption::Range {
            from_block,
            to_block,
        } => {
            let latest = execution
                .get_block(BlockId::latest(), false)
                .await?
                .ok_or(eyre!("latest block not found"))?;
            let to = to_block.unwrap_or(BlockNumberOrTag::Latest);
            let block = if to.is_latest() {
                latest.clone()
            } else {
                execution
                    .get_block(to.into(), false)
                    .await?
                    .ok_or(eyre!("end block not found"))?
            };
            let from = match from_block.unwrap_or(BlockNumberOrTag::Latest) {
                BlockNumberOrTag::Number(number) => number,
                BlockNumberOrTag::Latest => latest.header().number(),
                tag => execution
                    .get_block(tag.into(), false)
                    .await?
                    .ok_or(eyre!("start block not found"))?
                    .header()
                    .number(),
            };
            (block, from)
        }
    };
    let to = block.header().number();
    let count = to
        .checked_sub(from)
        .and_then(|n| n.checked_add(1))
        .ok_or(eyre!("invalid log range"))?;
    if count > MAX_SUPPORTED_BLOCKS_TO_PROVE_FOR_LOGS as u64 {
        return Err(eyre!(
            "log range exceeds {} blocks",
            MAX_SUPPORTED_BLOCKS_TO_PROVE_FOR_LOGS
        ));
    }
    let mut block_logs = Vec::new();
    for number in (from..=to).rev() {
        if block.header().number() != number {
            return Err(eyre!("inconsistent log block history"));
        }
        if filter.matches_bloom(block.header().logs_bloom()) {
            let receipts = execution
                .get_block_receipts(block.header().hash().into())
                .await?
                .ok_or(eyre!("block receipts not found"))?;
            let logs = receipts
                .iter()
                .flat_map(N::receipt_logs)
                .filter(|log| filter.rpc_matches(log))
                .collect::<Vec<_>>();
            block_logs.push(logs);
        }
        if number > from {
            block = execution
                .get_block(block.header().parent_hash().into(), false)
                .await?
                .ok_or(eyre!("parent block not found"))?;
        }
    }
    Ok(block_logs.into_iter().rev().flatten().collect())
}
