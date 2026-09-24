use alloy::eips::{BlockId, BlockNumberOrTag};
use alloy::network::Network;
use alloy::primitives::Address;
use alloy::rpc::types::{Block, BlockTransactions, Transaction};
use async_trait::async_trait;
use eyre::{eyre, Result};

use helios_common::execution_provider::{AccountProvider, BlockProvider};
use helios_core::execution::providers::historical::HistoricalBlockProvider;

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

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
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
            .get_untrusted_block(block_id, true)
            .await?;

        let Some(mut block) = block else {
            return Ok(None);
        };

        let matches_request = match block_id {
            BlockId::Hash(hash) => {
                // A sequencer signature alone cannot prove current canonicality.
                if hash.require_canonical == Some(true) {
                    return Err(eyre!("historical Linea canonicality cannot be verified"));
                }
                block.header.hash == hash.block_hash
            }
            BlockId::Number(BlockNumberOrTag::Number(number)) => block.header.number == number,
            _ => false,
        };
        if !matches_request {
            return Err(eyre!("historical block does not match requested block"));
        }

        // Since Linea uses the Ethereum spec, BlockResponse is Block<Transaction>
        // We can directly use it with our verify_block function
        match self.verify_linea_block(&block) {
            Ok(()) => {
                if !full_tx {
                    block.transactions =
                        BlockTransactions::Hashes(block.transactions.hashes().collect());
                }
                Ok(Some(block))
            }
            Err(e) => Err(eyre!("Linea block validation failed: {}", e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::B256;
    use helios_common::types::Account;

    struct Rpc(Block<Transaction>);
    #[async_trait]
    impl BlockProvider<Linea> for Rpc {
        async fn get_block(&self, _: BlockId, _: bool) -> Result<Option<Block<Transaction>>> {
            unreachable!()
        }
        async fn get_untrusted_block(
            &self,
            _: BlockId,
            full: bool,
        ) -> Result<Option<Block<Transaction>>> {
            assert!(full, "historical verification needs the full body");
            Ok(Some(self.0.clone()))
        }
        async fn push_block(&self, _: Block<Transaction>, _: BlockId) {
            unreachable!()
        }
    }
    #[async_trait]
    impl AccountProvider<Linea> for Rpc {
        async fn get_account(
            &self,
            _: Address,
            _: &[B256],
            _: bool,
            _: BlockId,
        ) -> Result<Account> {
            unreachable!()
        }
    }
    #[tokio::test]
    async fn rejects_a_different_requested_block_before_signature_verification() {
        let provider = LineaHistoricalProvider::new(Address::ZERO);
        let mut block: Block<Transaction> = Block::default();
        block.header.hash = B256::repeat_byte(1);
        let rpc = Rpc(block);
        for id in [B256::ZERO.into(), BlockId::number(1)] {
            let err = provider
                .get_historical_block(id, false, &rpc)
                .await
                .unwrap_err();
            assert!(err.to_string().contains("does not match requested block"));
        }
    }
}
