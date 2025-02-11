use std::collections::{HashMap, HashSet};

use alloy::consensus::BlockHeader;
use alloy::eips::BlockId;
use alloy::network::{BlockResponse, ReceiptResponse};
use alloy::primitives::{keccak256, Address, B256, U256};
use alloy::rlp;
use alloy::rpc::types::{Filter, FilterChanges, Log};
use async_trait::async_trait;
use eyre::Result;
use futures::future::try_join_all;
use revm::primitives::KECCAK_EMPTY;

use helios_common::{
    network_spec::NetworkSpec,
    types::{Account, BlockTag},
};

use crate::execution::constants::MAX_SUPPORTED_LOGS_NUMBER;
use crate::execution::errors::ExecutionError;
use crate::execution::proof::{
    ordered_trie_root_noop_encoder, verify_account_proof, verify_storage_proof,
};
use crate::execution::rpc::ExecutionRpc;
use crate::execution::state::State;

use super::VerifiableMethods;

#[derive(Clone)]
pub struct VerifiableMethodsRpc<N: NetworkSpec, R: ExecutionRpc<N>> {
    rpc: R,
    state: State<N, R>,
}

#[async_trait]
impl<N: NetworkSpec, R: ExecutionRpc<N>> VerifiableMethods<N, R> for VerifiableMethodsRpc<N, R> {
    fn new(url: &str, state: State<N, R>) -> Result<Self> {
        let rpc: R = ExecutionRpc::new(url)?;
        Ok(Self { rpc, state })
    }

    async fn get_account(
        &self,
        address: Address,
        slots: Option<&[B256]>,
        tag: BlockTag,
    ) -> Result<Account> {
        let slots = slots.unwrap_or(&[]);
        let block = self
            .state
            .get_block(tag)
            .await
            .ok_or(ExecutionError::BlockNotFound(tag))?;

        let proof = self
            .rpc
            .get_proof(address, slots, block.header().number().into())
            .await?;

        // Verify the account proof
        verify_account_proof(&proof, block.header().state_root())?;
        // Verify the storage proofs, collecting the slot values
        let slot_map = verify_storage_proof(&proof)?;
        // Verify the code hash
        let code = if proof.code_hash == KECCAK_EMPTY || proof.code_hash == B256::ZERO {
            Vec::new()
        } else {
            let code = self.rpc.get_code(address, block.header().number()).await?;
            let code_hash = keccak256(&code);

            if proof.code_hash != code_hash {
                return Err(
                    ExecutionError::CodeHashMismatch(address, code_hash, proof.code_hash).into(),
                );
            }

            code
        };

        Ok(Account {
            balance: proof.balance,
            nonce: proof.nonce,
            code,
            code_hash: proof.code_hash,
            storage_hash: proof.storage_hash,
            slots: slot_map,
        })
    }

    async fn get_transaction_receipt(&self, tx_hash: B256) -> Result<Option<N::ReceiptResponse>> {
        let receipt = self.rpc.get_transaction_receipt(tx_hash).await?;
        if receipt.is_none() {
            return Ok(None);
        }
        let receipt = receipt.unwrap();

        let block_number = receipt.block_number().unwrap();
        let block_id = BlockId::from(block_number);
        let tag = BlockTag::Number(block_number);

        let block = self.state.get_block(tag).await;
        let block = if let Some(block) = block {
            block
        } else {
            return Ok(None);
        };

        // Fetch all receipts in block, check root and inclusion
        let receipts = self
            .rpc
            .get_block_receipts(block_id)
            .await?
            .ok_or(eyre::eyre!(ExecutionError::NoReceiptsForBlock(tag)))?;

        let receipts_encoded = receipts.iter().map(N::encode_receipt).collect::<Vec<_>>();
        let expected_receipt_root = ordered_trie_root_noop_encoder(&receipts_encoded);

        if expected_receipt_root != block.header().receipts_root()
            // Note: Some RPC providers return different response in `eth_getTransactionReceipt` vs `eth_getBlockReceipts`
            // Primarily due to https://github.com/ethereum/execution-apis/issues/295 not finalized
            // Which means that the basic equality check in N::receipt_contains can be flaky
            // So as a fallback do equality check on encoded receipts as well
            || !(
                N::receipt_contains(&receipts, &receipt)
                || receipts_encoded.contains(&N::encode_receipt(&receipt))
            )
        {
            return Err(ExecutionError::ReceiptRootMismatch(tx_hash).into());
        }

        Ok(Some(receipt))
    }

    async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>> {
        let logs = self.rpc.get_logs(filter).await?;

        if logs.len() > MAX_SUPPORTED_LOGS_NUMBER {
            return Err(
                ExecutionError::TooManyLogsToProve(logs.len(), MAX_SUPPORTED_LOGS_NUMBER).into(),
            );
        }

        self.verify_logs(&logs).await?;
        Ok(logs)
    }

    async fn get_filter_changes(&self, filter_id: U256) -> Result<FilterChanges> {
        let filter_changes = self.rpc.get_filter_changes(filter_id).await?;

        if filter_changes.is_logs() {
            let logs = filter_changes.as_logs().unwrap_or(&[]);
            if logs.len() > MAX_SUPPORTED_LOGS_NUMBER {
                return Err(ExecutionError::TooManyLogsToProve(
                    logs.len(),
                    MAX_SUPPORTED_LOGS_NUMBER,
                )
                .into());
            }
            self.verify_logs(logs).await?;
        }

        Ok(filter_changes)
    }

    async fn get_filter_logs(&self, filter_id: U256) -> Result<Vec<Log>> {
        let logs = self.rpc.get_filter_logs(filter_id).await?;

        if logs.len() > MAX_SUPPORTED_LOGS_NUMBER {
            return Err(
                ExecutionError::TooManyLogsToProve(logs.len(), MAX_SUPPORTED_LOGS_NUMBER).into(),
            );
        }

        self.verify_logs(&logs).await?;
        Ok(logs)
    }
}

impl<N: NetworkSpec, R: ExecutionRpc<N>> VerifiableMethodsRpc<N, R> {
    /// Verify the integrity of each log entry in the given array of logs by
    /// checking its inclusion in the corresponding transaction receipt
    /// and verifying the transaction receipt itself against the block's receipt root.
    async fn verify_logs(&self, logs: &[Log]) -> Result<()> {
        // Collect all (unique) block numbers
        let block_nums = logs
            .iter()
            .map(|log| {
                log.block_number
                    .ok_or_else(|| eyre::eyre!("block num not found in log"))
            })
            .collect::<Result<HashSet<_>, _>>()?;

        // Collect all (proven) tx receipts for all block numbers
        let blocks_receipts_fut = block_nums.into_iter().map(|block_num| async move {
            let tag = BlockTag::Number(block_num);
            // ToDo(@eshaan7): use verified version of `get_block_receipts`
            let receipts = self.rpc.get_block_receipts(block_num.into()).await;
            receipts?.ok_or_else(|| eyre::eyre!(ExecutionError::NoReceiptsForBlock(tag)))
        });
        let blocks_receipts = try_join_all(blocks_receipts_fut).await?;
        let receipts = blocks_receipts.into_iter().flatten().collect::<Vec<_>>();

        // Map tx hashes to encoded logs
        let receipts_logs_encoded = receipts
            .into_iter()
            .filter_map(|receipt| {
                let logs = N::receipt_logs(&receipt);
                if logs.is_empty() {
                    None
                } else {
                    let tx_hash = logs[0].transaction_hash.unwrap();
                    let encoded_logs = logs
                        .iter()
                        .map(|l| rlp::encode(&l.inner))
                        .collect::<Vec<_>>();
                    Some((tx_hash, encoded_logs))
                }
            })
            .collect::<HashMap<_, _>>();

        for log in logs {
            // Check if the receipt contains the desired log
            // Encoding logs for comparison
            let tx_hash = log.transaction_hash.unwrap();
            let log_encoded = rlp::encode(&log.inner);
            let receipt_logs_encoded = receipts_logs_encoded.get(&tx_hash).unwrap();

            if !receipt_logs_encoded.contains(&log_encoded) {
                return Err(ExecutionError::MissingLog(
                    tx_hash,
                    U256::from(log.log_index.unwrap()),
                )
                .into());
            }
        }
        Ok(())
    }
}
