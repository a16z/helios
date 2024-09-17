use std::collections::HashMap;

use alloy::consensus::{Receipt, ReceiptWithBloom, TxReceipt, TxType};
use alloy::primitives::{keccak256, Address, B256, U256};
use alloy::rlp::encode;
use alloy::rpc::types::{Filter, Log, Transaction, TransactionReceipt};
use anyhow::Result;
use futures::future::join_all;
use revm::primitives::KECCAK_EMPTY;
use triehash_ethereum::ordered_trie_root;

use common::errors::BlockNotFoundError;
use common::types::{Block, BlockTag, Transactions};

use crate::constants::MAX_SUPPORTED_LOGS_NUMBER;
use crate::errors::ExecutionError;
use crate::state::State;

use super::proof::{encode_account, verify_proof};
use super::rpc::ExecutionRpc;
use super::types::Account;

#[derive(Clone)]
pub struct ExecutionClient<R: ExecutionRpc> {
    pub rpc: R,
    state: State,
}

impl<R: ExecutionRpc> ExecutionClient<R> {
    pub fn new(rpc: &str, state: State) -> Result<Self> {
        let rpc: R = ExecutionRpc::new(rpc)?;
        Ok(ExecutionClient { rpc, state })
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
            .ok_or(BlockNotFoundError::new(tag))
            .map_err(|e| anyhow::anyhow!(e))?;

        let proof = self
            .rpc
            .get_proof(address, slots, block.number.to())
            .await?;

        let account_path = keccak256(address).to_vec();
        let account_encoded = encode_account(&proof);

        let is_valid = verify_proof(
            &proof.account_proof,
            block.state_root.as_slice(),
            &account_path,
            &account_encoded,
        );

        if !is_valid {
            return Err(ExecutionError::InvalidAccountProof(address).into());
        }

        let mut slot_map = HashMap::new();

        for storage_proof in proof.storage_proof {
            let key = storage_proof.key.0;
            let key_hash = keccak256(key);
            let value = encode(storage_proof.value);

            let is_valid = verify_proof(
                &storage_proof.proof,
                proof.storage_hash.as_slice(),
                key_hash.as_slice(),
                &value,
            );

            if !is_valid {
                return Err(ExecutionError::InvalidStorageProof(address, key).into());
            }

            slot_map.insert(key, storage_proof.value);
        }

        let code = if proof.code_hash == KECCAK_EMPTY {
            Vec::new()
        } else {
            let code = self.rpc.get_code(address, block.number.to()).await?;
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

    pub async fn get_block(&self, tag: BlockTag, full_tx: bool) -> Result<Block> {
        let mut block = self
            .state
            .get_block(tag)
            .await
            .ok_or(BlockNotFoundError::new(tag))
            .map_err(|e| anyhow::anyhow!(e))?;

        if !full_tx {
            block.transactions = Transactions::Hashes(block.transactions.hashes());
        }

        Ok(block)
    }

    pub async fn get_block_by_hash(&self, hash: B256, full_tx: bool) -> Result<Block> {
        let mut block = self
            .state
            .get_block_by_hash(hash)
            .await
            .ok_or(anyhow::anyhow!("block not found"))?;

        if !full_tx {
            block.transactions = Transactions::Hashes(block.transactions.hashes());
        }

        Ok(block)
    }

    pub async fn get_transaction_by_block_hash_and_index(
        &self,
        block_hash: B256,
        index: u64,
    ) -> Option<Transaction> {
        self.state
            .get_transaction_by_block_and_index(block_hash, index)
            .await
    }

    pub async fn get_transaction_receipt(
        &self,
        tx_hash: B256,
    ) -> Result<Option<TransactionReceipt>> {
        let receipt = self.rpc.get_transaction_receipt(tx_hash).await?;
        if receipt.is_none() {
            return Ok(None);
        }

        let receipt = receipt.unwrap();
        let block_number = receipt.block_number.unwrap();

        let block = self.state.get_block(BlockTag::Number(block_number)).await;
        let block = if let Some(block) = block {
            block
        } else {
            return Ok(None);
        };

        let tx_hashes = block.transactions.hashes();

        let receipts_fut = tx_hashes.iter().map(|hash| async move {
            let receipt = self.rpc.get_transaction_receipt(*hash).await;
            receipt?.ok_or(anyhow::anyhow!("not reachable"))
        });

        let receipts = join_all(receipts_fut).await;
        let receipts = receipts.into_iter().collect::<Result<Vec<_>>>()?;
        let receipts_encoded: Vec<Vec<u8>> = receipts.iter().map(encode_receipt).collect();

        let expected_receipt_root = ordered_trie_root(receipts_encoded);
        let expected_receipt_root = B256::from_slice(&expected_receipt_root.to_fixed_bytes());

        if expected_receipt_root != block.receipts_root || !receipts.contains(&receipt) {
            return Err(ExecutionError::ReceiptRootMismatch(tx_hash).into());
        }

        Ok(Some(receipt))
    }

    pub async fn get_transaction(&self, hash: B256) -> Option<Transaction> {
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

    pub async fn get_filter_changes(&self, filter_id: U256) -> Result<Vec<Log>> {
        let logs = self.rpc.get_filter_changes(filter_id).await?;
        if logs.len() > MAX_SUPPORTED_LOGS_NUMBER {
            return Err(
                ExecutionError::TooManyLogsToProve(logs.len(), MAX_SUPPORTED_LOGS_NUMBER).into(),
            );
        }
        self.verify_logs(&logs).await?;
        Ok(logs)
    }

    pub async fn uninstall_filter(&self, filter_id: U256) -> Result<bool> {
        self.rpc.uninstall_filter(filter_id).await
    }

    pub async fn get_new_filter(&self, filter: &Filter) -> Result<U256> {
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
        self.rpc.get_new_filter(&filter).await
    }

    pub async fn get_new_block_filter(&self) -> Result<U256> {
        self.rpc.get_new_block_filter().await
    }

    pub async fn get_new_pending_transaction_filter(&self) -> Result<U256> {
        self.rpc.get_new_pending_transaction_filter().await
    }

    async fn verify_logs(&self, logs: &[Log]) -> Result<()> {
        for log in logs.iter() {
            // For every log
            // Get the hash of the tx that generated it
            let tx_hash = log
                .transaction_hash
                .ok_or(anyhow::anyhow!("tx hash not found in log"))?;

            // Get its proven receipt
            let receipt = self
                .get_transaction_receipt(tx_hash)
                .await?
                .ok_or(ExecutionError::NoReceiptForTransaction(tx_hash))?;

            // Check if the receipt contains the desired log
            // Encoding logs for comparison
            let receipt_logs_encoded = receipt
                .inner
                .logs()
                .iter()
                .map(|log| encode(&log.inner))
                .collect::<Vec<_>>();

            let log_encoded = encode(&log.inner);

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

fn encode_receipt(receipt: &TransactionReceipt) -> Vec<u8> {
    let tx_type = receipt.transaction_type();
    let receipt = receipt.inner.as_receipt_with_bloom().unwrap();
    let logs = receipt
        .logs()
        .iter()
        .map(|l| l.inner.clone())
        .collect::<Vec<_>>();

    let consensus_receipt = Receipt {
        cumulative_gas_used: receipt.cumulative_gas_used(),
        status: *receipt.status_or_post_state(),
        logs,
    };

    let rwb = ReceiptWithBloom::new(consensus_receipt, receipt.bloom());
    let encoded = alloy::rlp::encode(rwb);

    match tx_type {
        TxType::Legacy => encoded,
        _ => [vec![tx_type as u8], encoded].concat(),
    }
}
