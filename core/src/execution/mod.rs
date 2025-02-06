use std::collections::{HashMap, HashSet};

use alloy::consensus::BlockHeader;
use alloy::network::primitives::HeaderResponse;
use alloy::network::{BlockResponse, ReceiptResponse};
use alloy::primitives::{keccak256, Address, B256, U256};
use alloy::rlp;
use alloy::rpc::types::{BlockTransactions, Filter, FilterChanges, Log};
use alloy_trie::{
    proof::verify_proof as mpt_verify_proof,
    root::ordered_trie_root_with_encoder,
    {Nibbles, TrieAccount},
};
use constants::{BLOB_BASE_FEE_UPDATE_FRACTION, MIN_BASE_FEE_PER_BLOB_GAS};
use eyre::Result;
use futures::future::try_join_all;
use revm::primitives::KECCAK_EMPTY;
use tracing::warn;

use crate::network_spec::NetworkSpec;
use crate::types::BlockTag;

use self::constants::MAX_SUPPORTED_LOGS_NUMBER;
use self::errors::ExecutionError;
use self::rpc::ExecutionRpc;
use self::state::{FilterType, State};
use self::types::Account;

pub mod constants;
pub mod errors;
pub mod evm;
pub mod rpc;
pub mod state;
pub mod types;

#[derive(Clone)]
pub struct ExecutionClient<N: NetworkSpec, R: ExecutionRpc<N>> {
    pub rpc: R,
    state: State<N, R>,
}

impl<N: NetworkSpec, R: ExecutionRpc<N>> ExecutionClient<N, R> {
    pub fn new(rpc: &str, state: State<N, R>) -> Result<Self> {
        let rpc: R = ExecutionRpc::new(rpc)?;
        Ok(ExecutionClient::<N, R> { rpc, state })
    }

    pub async fn check_rpc(&self, chain_id: u64) -> Result<()> {
        if self.rpc.chain_id().await? != chain_id {
            Err(ExecutionError::IncorrectRpcNetwork().into())
        } else {
            Ok(())
        }
    }

    pub async fn get_account(
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
        let account_key = Nibbles::unpack(keccak256(proof.address));
        let account = TrieAccount {
            nonce: proof.nonce,
            balance: proof.balance,
            storage_root: proof.storage_hash,
            code_hash: proof.code_hash,
        };
        let account_encoded = rlp::encode(account);

        mpt_verify_proof(
            block.header().state_root(),
            account_key,
            account_encoded.into(),
            &proof.account_proof,
        )
        .map_err(|_| ExecutionError::InvalidAccountProof(address))?;

        // Verify the storage proofs, collecting the slot values
        let mut slot_map = HashMap::new();

        for storage_proof in proof.storage_proof {
            let key = storage_proof.key.as_b256();
            let key_hash = keccak256(key);
            let key_nibbles = Nibbles::unpack(key_hash);
            let encoded_value = rlp::encode(storage_proof.value);

            mpt_verify_proof(
                proof.storage_hash,
                key_nibbles,
                encoded_value.into(),
                &storage_proof.proof,
            )
            .map_err(|_| ExecutionError::InvalidStorageProof(address, key))?;

            slot_map.insert(key, storage_proof.value);
        }

        // Verify the code hash
        let code = if proof.code_hash == KECCAK_EMPTY || proof.code_hash == B256::ZERO {
            Vec::new()
        } else {
            let code = self
                .rpc
                .get_code(address, block.header().number().into())
                .await?;
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

    pub async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<B256> {
        self.rpc.send_raw_transaction(bytes).await
    }

    pub async fn get_block(&self, tag: BlockTag, full_tx: bool) -> Option<N::BlockResponse> {
        let block = self.state.get_block(tag).await;
        if block.is_none() {
            warn!(target: "helios::execution", "requested block not found in state: {}", tag);
            return None;
        }
        let mut block = block.unwrap();

        if !full_tx {
            *block.transactions_mut() =
                BlockTransactions::Hashes(block.transactions().hashes().collect());
        }

        Some(block)
    }

    pub async fn blob_base_fee(&self, block: BlockTag) -> U256 {
        let block = self.state.get_block(block).await;
        if block.is_none() {
            warn!(target: "helios::execution", "requested block not found");
            return U256::from(0);
        }
        let parent_hash = block.unwrap().header().parent_hash();
        let parent_block = self.get_block_by_hash(parent_hash, false).await;
        if parent_block.is_none() {
            warn!(target: "helios::execution", "requested parent block not foundß");
            return U256::from(0);
        };

        let blob_base_fee = Self::calculate_base_fee_per_blob_gas(
            parent_block.unwrap().header().excess_blob_gas().unwrap(),
        );

        U256::from(blob_base_fee)
    }

    fn calculate_base_fee_per_blob_gas(parent_excess_blob_gas: u64) -> u64 {
        Self::fake_exponential(
            MIN_BASE_FEE_PER_BLOB_GAS,
            parent_excess_blob_gas,
            BLOB_BASE_FEE_UPDATE_FRACTION,
        )
    }
    //https://github.com/ethereum/EIPs/blob/master/EIPS/eip-4844.md#helpers
    fn fake_exponential(factor: u64, numerator: u64, denominator: u64) -> u64 {
        let mut i = 1;
        let mut output = 0;
        let mut numerator_accum = factor * denominator;
        while numerator_accum > 0 {
            output += numerator_accum;
            numerator_accum = numerator_accum * numerator / (denominator * i);
            i += 1;
        }
        output / denominator
    }

    pub async fn get_block_by_hash(&self, hash: B256, full_tx: bool) -> Option<N::BlockResponse> {
        let block = self.state.get_block_by_hash(hash).await;
        if block.is_none() {
            warn!(target: "helios::execution", "requested block not found in state: {}", hash);
            return None;
        }
        let mut block = block.unwrap();

        if !full_tx {
            *block.transactions_mut() =
                BlockTransactions::Hashes(block.transactions().hashes().collect());
        }

        Some(block)
    }

    pub async fn get_transaction_by_block_hash_and_index(
        &self,
        block_hash: B256,
        index: u64,
    ) -> Option<N::TransactionResponse> {
        self.state
            .get_transaction_by_block_hash_and_index(block_hash, index)
            .await
    }

    pub async fn get_transaction_by_block_number_and_index(
        &self,
        tag: BlockTag,
        index: u64,
    ) -> Option<N::TransactionResponse> {
        self.state
            .get_transaction_by_block_and_index(tag, index)
            .await
    }

    pub async fn get_transaction_receipt(
        &self,
        tx_hash: B256,
    ) -> Result<Option<N::ReceiptResponse>> {
        let receipt = self.rpc.get_transaction_receipt(tx_hash).await?;
        if receipt.is_none() {
            return Ok(None);
        }
        let receipt = receipt.unwrap();

        let block_number = receipt.block_number().unwrap();
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
            .get_block_receipts(tag)
            .await?
            .ok_or(eyre::eyre!(ExecutionError::NoReceiptsForBlock(tag)))?;

        let receipts_encoded = receipts.iter().map(N::encode_receipt).collect::<Vec<_>>();
        let expected_receipt_root = ordered_trie_root(&receipts_encoded);

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

    pub async fn get_block_receipts(
        &self,
        tag: BlockTag,
    ) -> Result<Option<Vec<N::ReceiptResponse>>> {
        let block = self.state.get_block(tag).await;
        let block = if let Some(block) = block {
            block
        } else {
            return Ok(None);
        };

        let tag = BlockTag::Number(block.header().number().into());

        let receipts = self
            .rpc
            .get_block_receipts(tag)
            .await?
            .ok_or(eyre::eyre!(ExecutionError::NoReceiptsForBlock(tag)))?;

        let receipts_encoded = receipts.iter().map(N::encode_receipt).collect::<Vec<_>>();
        let expected_receipt_root = ordered_trie_root(&receipts_encoded);

        if expected_receipt_root != block.header().receipts_root() {
            return Err(ExecutionError::BlockReceiptsRootMismatch(tag).into());
        }

        Ok(Some(receipts))
    }

    pub async fn get_transaction(&self, hash: B256) -> Option<N::TransactionResponse> {
        self.state.get_transaction(hash).await
    }

    pub async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>> {
        let filter = filter.clone();

        // avoid fetching logs for a block helios hasn't seen yet
        let filter = if filter.get_to_block().is_none() && filter.get_block_hash().is_none() {
            let block = self.state.latest_block_number().await.unwrap();
            let filter = filter.to_block(block);
            if filter.get_from_block().is_none() {
                filter.from_block(block)
            } else {
                filter
            }
        } else {
            filter
        };

        let logs = self.rpc.get_logs(&filter).await?;
        if logs.len() > MAX_SUPPORTED_LOGS_NUMBER {
            return Err(
                ExecutionError::TooManyLogsToProve(logs.len(), MAX_SUPPORTED_LOGS_NUMBER).into(),
            );
        }

        self.verify_logs(&logs).await?;
        Ok(logs)
    }

    pub async fn get_filter_changes(&self, filter_id: U256) -> Result<FilterChanges> {
        let filter_type = self.state.get_filter(&filter_id).await;

        Ok(match &filter_type {
            None => {
                // only concerned with filters created via helios
                return Err(ExecutionError::FilterNotFound(filter_id).into());
            }
            Some(FilterType::Logs) => {
                // underlying RPC takes care of keeping track of changes
                let filter_changes = self.rpc.get_filter_changes(filter_id).await?;
                let logs = filter_changes.as_logs().unwrap_or(&[]);
                if logs.len() > MAX_SUPPORTED_LOGS_NUMBER {
                    return Err(ExecutionError::TooManyLogsToProve(
                        logs.len(),
                        MAX_SUPPORTED_LOGS_NUMBER,
                    )
                    .into());
                }
                self.verify_logs(logs).await?;
                FilterChanges::Logs(logs.to_vec())
            }
            Some(FilterType::NewBlock(last_block_num)) => {
                let blocks = self
                    .state
                    .get_blocks_after(BlockTag::Number(*last_block_num))
                    .await;
                if !blocks.is_empty() {
                    // keep track of the last block number in state
                    // so next call can filter starting from the prev call's (last block number + 1)
                    self.state
                        .push_filter(
                            filter_id,
                            FilterType::NewBlock(blocks.last().unwrap().header().number().into()),
                        )
                        .await;
                }
                let block_hashes = blocks.into_iter().map(|b| b.header().hash()).collect();
                FilterChanges::Hashes(block_hashes)
            }
            Some(FilterType::PendingTransactions) => {
                // underlying RPC takes care of keeping track of changes
                let filter_changes = self.rpc.get_filter_changes(filter_id).await?;
                let tx_hashes = filter_changes.as_hashes().unwrap_or(&[]);
                FilterChanges::Hashes(tx_hashes.to_vec())
            }
        })
    }

    pub async fn get_filter_logs(&self, filter_id: U256) -> Result<Vec<Log>> {
        let logs = self.rpc.get_filter_logs(filter_id).await?;
        if logs.len() > MAX_SUPPORTED_LOGS_NUMBER {
            return Err(
                ExecutionError::TooManyLogsToProve(logs.len(), MAX_SUPPORTED_LOGS_NUMBER).into(),
            );
        }
        self.verify_logs(&logs).await?;
        Ok(logs)
    }

    pub async fn uninstall_filter(&self, filter_id: U256) -> Result<bool> {
        // remove the filter from the state
        self.state.remove_filter(&filter_id).await;
        self.rpc.uninstall_filter(filter_id).await
    }

    pub async fn new_filter(&self, filter: &Filter) -> Result<U256> {
        let filter = filter.clone();

        // avoid submitting a filter for logs for a block helios hasn't seen yet
        let filter = if filter.get_to_block().is_none() && filter.get_block_hash().is_none() {
            let block = self.state.latest_block_number().await.unwrap();
            let filter = filter.to_block(block);
            if filter.get_from_block().is_none() {
                filter.from_block(block)
            } else {
                filter
            }
        } else {
            filter
        };
        let filter_id = self.rpc.new_filter(&filter).await?;

        // record the filter in the state
        self.state.push_filter(filter_id, FilterType::Logs).await;

        Ok(filter_id)
    }

    pub async fn new_block_filter(&self) -> Result<U256> {
        let filter_id = self.rpc.new_block_filter().await?;

        // record the filter in the state
        let latest_block_num = self.state.latest_block_number().await.unwrap_or(1);
        self.state
            .push_filter(filter_id, FilterType::NewBlock(latest_block_num))
            .await;

        Ok(filter_id)
    }

    pub async fn new_pending_transaction_filter(&self) -> Result<U256> {
        let filter_id = self.rpc.new_pending_transaction_filter().await?;

        // record the filter in the state
        self.state
            .push_filter(filter_id, FilterType::PendingTransactions)
            .await;

        Ok(filter_id)
    }

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
            let receipts = self.get_block_receipts(tag).await;
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

/// Compute a trie root of a collection of encoded items.
/// Ref: https://github.com/alloy-rs/trie/blob/main/src/root.rs.
fn ordered_trie_root(items: &[Vec<u8>]) -> B256 {
    fn noop_encoder(item: &Vec<u8>, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(item);
    }

    ordered_trie_root_with_encoder(items, noop_encoder)
}
