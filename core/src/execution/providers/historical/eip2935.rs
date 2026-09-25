use std::marker::PhantomData;

use alloy::consensus::BlockHeader;
use alloy::eips::{BlockId, BlockNumberOrTag};
use alloy::network::{primitives::HeaderResponse, BlockResponse};
use alloy::primitives::{address, B256, U256};
use async_trait::async_trait;
use eyre::{eyre, Result};

use helios_common::{
    execution_provider::{AccountProvider, BlockProvider},
    network_spec::NetworkSpec,
};

use super::HistoricalBlockProvider;

/// EIP-2935 historical block provider for Ethereum and OpStack chains.
///
/// This provider retrieves historical blocks using the EIP-2935 block hash history contract
/// which stores the last 8191 block hashes in a ring buffer at address 0x0000...2935.
pub struct Eip2935Provider<N: NetworkSpec> {
    ring_buffer_size: u64,
    contract_address: alloy::primitives::Address,
    _phantom: PhantomData<N>,
}

impl<N: NetworkSpec> Default for Eip2935Provider<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<N: NetworkSpec> Eip2935Provider<N> {
    pub fn new() -> Self {
        Self {
            ring_buffer_size: 8191,
            contract_address: address!("0000F90827F1C53a10cb7A02335B175320002935"),
            _phantom: PhantomData,
        }
    }

    /// Create a new EIP-2935 provider with custom configuration
    pub fn with_config(
        ring_buffer_size: u64,
        contract_address: alloy::primitives::Address,
    ) -> Self {
        Self {
            ring_buffer_size,
            contract_address,
            _phantom: PhantomData,
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec> HistoricalBlockProvider<N> for Eip2935Provider<N> {
    async fn get_historical_block<E>(
        &self,
        block_id: BlockId,
        full_tx: bool,
        execution_provider: &E,
    ) -> Result<Option<N::BlockResponse>>
    where
        E: BlockProvider<N> + AccountProvider<N>,
    {
        // Get the untrusted block from the execution provider first
        // This works for both block numbers and block hashes
        let target_block = execution_provider
            .get_untrusted_block(block_id, true)
            .await?;

        let Some(mut target_block) = target_block else {
            return Ok(None);
        };

        // Extract the block number from the fetched block (works for both number and hash queries)
        let target_number = target_block.header().number();

        let matches_request = match block_id {
            BlockId::Hash(hash) => target_block.header().hash() == hash.block_hash,
            BlockId::Number(BlockNumberOrTag::Number(number)) => target_number == number,
            _ => false,
        };
        if !matches_request {
            return Err(eyre!("historical block does not match requested block"));
        }

        // Get the trusted latest block from the execution provider
        let latest_block = execution_provider
            .get_block(BlockId::Number(BlockNumberOrTag::Latest), false)
            .await?;

        let Some(latest_block) = latest_block else {
            return Err(eyre!("failed to get latest trusted block"));
        };

        let latest_number = latest_block.header().number();
        let latest_hash = latest_block.header().hash();

        // Check if the target block is within the EIP-2935 ring buffer range
        // The ring buffer stores the last `ring_buffer_size` block hashes
        let distance = latest_number.checked_sub(target_number);
        if !distance.is_some_and(|distance| distance > 0 && distance <= self.ring_buffer_size) {
            return Err(eyre!(
                "block {} is outside EIP-2935 ring buffer range (latest: {}, buffer size: {})",
                target_number,
                latest_number,
                self.ring_buffer_size
            ));
        }

        // Calculate the storage slot in the EIP-2935 contract
        let slot = B256::from(U256::from(target_number % self.ring_buffer_size));

        // Retrieve and verify the stored block hash from the EIP-2935 contract using verified account access
        let account = execution_provider
            .get_account(self.contract_address, &[slot], false, latest_hash.into())
            .await?;

        let stored_hash = account.get_storage_value(slot).ok_or(eyre!(
            "failed to retrieve block hash from EIP-2935 contract"
        ))?;

        let stored_hash = B256::from(stored_hash);

        // Reject the wrong header before doing transaction hashing or sender recovery.
        if target_block.header().hash() != stored_hash
            || !N::validate_block(&mut target_block, full_tx)
        {
            return Err(eyre!(
                "block validation failed: hash mismatch or invalid block structure"
            ));
        }
        Ok(Some(target_block))
    }
}
