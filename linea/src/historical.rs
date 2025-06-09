use alloy::eips::BlockId;
use alloy::network::Network;
use alloy::primitives::Address;
use alloy::rpc::types::{Block, Transaction};
use async_trait::async_trait;
use eyre::{eyre, Result};

use helios_core::execution::providers::historical::HistoricalBlockProvider;
use helios_core::execution::providers::{AccountProvider, BlockProvider};

use crate::consensus::verify_block;
use crate::spec::Linea;

/// Linea historical block provider using extradata signature validation.
///
/// This provider validates historical Linea blocks by verifying the signature
/// in the extradata field against the configured unsafe signer address.
/// Uses the same validation mechanism as Linea consensus.
pub struct LineaHistoricalProvider {
    unsafe_signer: Address,
}

impl LineaHistoricalProvider {
    pub fn new(unsafe_signer: Address) -> Self {
        Self { unsafe_signer }
    }

    /// Verify a Linea block by checking the signature in the extradata field
    /// This reuses the same logic as the Linea consensus verify_block function
    fn verify_linea_block(&self, block: &Block<Transaction>) -> Result<()> {
        verify_block(self.unsafe_signer, block)
    }
}

#[async_trait]
impl HistoricalBlockProvider<Linea> for LineaHistoricalProvider {
    async fn get_historical_block<E>(
        &self,
        block_id: BlockId,
        full_tx: bool,
        execution_provider: &E,
    ) -> Result<Option<<Linea as Network>::BlockResponse>>
    where
        E: BlockProvider<Linea> + AccountProvider<Linea>,
    {
        // Get the untrusted block from execution provider
        // This works for both block numbers and block hashes
        let block = execution_provider
            .get_untrusted_block(block_id, full_tx)
            .await?;

        let Some(block) = block else {
            return Ok(None);
        };

        // Since Linea uses the Ethereum spec, BlockResponse is Block<Transaction>
        // We can directly use it with our verify_block function
        match self.verify_linea_block(&block) {
            Ok(()) => Ok(Some(block)),
            Err(e) => Err(eyre!("Linea block validation failed: {}", e)),
        }
    }
}
