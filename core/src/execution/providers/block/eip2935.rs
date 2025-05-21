use alloy::consensus::BlockHeader;
use alloy::eips::{BlockId, BlockNumberOrTag};
use alloy::network::{primitives::HeaderResponse, BlockResponse};
use alloy::primitives::{address, B256, U256};
use eyre::{eyre, Result};

use helios_common::network_spec::NetworkSpec;

use crate::execution::providers::{AccountProvider, BlockProvider};

pub async fn get_block<N: NetworkSpec, P: BlockProvider<N> + AccountProvider<N>>(
    block_id: BlockId,
    full_tx: bool,
    provider: &P,
) -> Result<N::BlockResponse> {
    let latest_block = provider
        .get_block(BlockId::Number(BlockNumberOrTag::Latest), false)
        .await?;

    let Some(latest_block) = latest_block else {
        return Err(eyre!("historical block fetch failed"));
    };

    let Some(target_block) = provider.get_untrusted_block(block_id, full_tx).await? else {
        return Err(eyre!("historical block fetch failed"));
    };

    let latest_hash = latest_block.header().hash();
    let latest_number = latest_block.header().number();
    let target_number = target_block.header().number();

    if target_number > latest_number - 8191 {
        let slot = B256::from(U256::from(target_number % 8191));
        let historical_block_address = address!("0000F90827F1C53a10cb7A02335B175320002935");
        let target_block_hash = provider
            .get_account(historical_block_address, &[slot], false, latest_hash.into())
            .await?
            .get_storage_value(slot)
            .ok_or(eyre!("historical block fetch failed"))?;

        let target_block_hash = B256::from(target_block_hash);
        let is_hash_valid = N::is_hash_valid(&target_block);

        if is_hash_valid && target_block.header().hash() == target_block_hash {
            Ok(target_block)
        } else {
            Err(eyre!("historical block fetch failed"))
        }
    } else {
        Err(eyre!("historical block fetch failed"))
    }
}
