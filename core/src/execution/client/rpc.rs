use std::collections::{HashMap, HashSet};

use alloy::consensus::BlockHeader;
use alloy::eips::BlockId;
use alloy::network::{BlockResponse, ReceiptResponse, TransactionBuilder};
use alloy::primitives::{keccak256, Address, B256, U256};
use alloy::rlp;
use alloy::rpc::types::{EIP1186AccountProofResponse, Filter, FilterChanges, Log};
use async_trait::async_trait;
use eyre::Result;
use futures::future::{join_all, try_join_all};
use revm::primitives::{AccessListItem, KECCAK_EMPTY};

use helios_common::{
    network_spec::NetworkSpec,
    types::{Account, BlockTag},
};

use crate::execution::constants::{
    MAX_SUPPORTED_BLOCKS_TO_PROVE_FOR_LOGS, PARALLEL_QUERY_BATCH_SIZE,
};
use crate::execution::errors::ExecutionError;
use crate::execution::proof::{
    ordered_trie_root_noop_encoder, verify_account_proof, verify_storage_proof,
};
use crate::execution::rpc::ExecutionRpc;
use crate::execution::state::State;

use super::ExecutionMethods;

#[derive(Clone)]
pub struct ExecutionRpcClient<N: NetworkSpec, R: ExecutionRpc<N>> {
    rpc: R,
    state: State<N, R>,
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec, R: ExecutionRpc<N>> ExecutionMethods<N, R> for ExecutionRpcClient<N, R> {
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

        self.verify_proof_to_account(&proof, &block).await
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

        self.verify_logs(&logs).await?;

        Ok(logs)
    }

    async fn get_filter_changes(&self, filter_id: U256) -> Result<FilterChanges> {
        let filter_changes = self.rpc.get_filter_changes(filter_id).await?;

        if filter_changes.is_logs() {
            let logs = filter_changes.as_logs().unwrap_or(&[]);
            self.verify_logs(logs).await?;
        }

        Ok(filter_changes)
    }

    async fn get_filter_logs(&self, filter_id: U256) -> Result<Vec<Log>> {
        let logs = self.rpc.get_filter_logs(filter_id).await?;

        self.verify_logs(&logs).await?;

        Ok(logs)
    }

    async fn create_access_list(
        &self,
        tx: &N::TransactionRequest,
        block: Option<BlockId>,
    ) -> Result<HashMap<Address, Account>> {
        let block_id = block.unwrap_or(BlockId::latest());
        let tag = BlockTag::try_from(block_id)?;
        let block = self
            .state
            .get_block(tag)
            .await
            .ok_or(ExecutionError::BlockNotFound(tag))?;
        let block_id = BlockId::Number(block.header().number().into());

        let mut list = self.rpc.create_access_list(tx, block_id).await?.0;

        let from_access_entry = AccessListItem {
            address: tx.from().unwrap_or_default(),
            storage_keys: Vec::default(),
        };
        let to_access_entry = AccessListItem {
            address: tx.to().unwrap_or_default(),
            storage_keys: Vec::default(),
        };
        let producer_access_entry = AccessListItem {
            address: block.header().beneficiary(),
            storage_keys: Vec::default(),
        };

        let list_addresses = list.iter().map(|elem| elem.address).collect::<Vec<_>>();

        if !list_addresses.contains(&from_access_entry.address) {
            list.push(from_access_entry)
        }
        if !list_addresses.contains(&to_access_entry.address) {
            list.push(to_access_entry)
        }
        if !list_addresses.contains(&producer_access_entry.address) {
            list.push(producer_access_entry)
        }

        let mut account_map = HashMap::new();
        for chunk in list.chunks(PARALLEL_QUERY_BATCH_SIZE) {
            let account_chunk_futs = chunk.iter().map(|account| {
                let account_fut =
                    self.get_account(account.address, Some(account.storage_keys.as_slice()), tag);
                async move { (account.address, account_fut.await) }
            });

            let account_chunk = join_all(account_chunk_futs).await;

            for (address, value) in account_chunk {
                let account = value?;
                account_map.insert(address, account);
            }
        }

        Ok(account_map)
    }

    async fn chain_id(&self) -> Result<u64> {
        self.rpc.chain_id().await
    }

    async fn get_block_receipts(&self, block: BlockId) -> Result<Option<Vec<N::ReceiptResponse>>> {
        self.rpc.get_block_receipts(block).await
    }

    async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<B256> {
        self.rpc.send_raw_transaction(bytes).await
    }

    async fn new_filter(&self, filter: &Filter) -> Result<U256> {
        self.rpc.new_filter(filter).await
    }

    async fn new_block_filter(&self) -> Result<U256> {
        self.rpc.new_block_filter().await
    }

    async fn new_pending_transaction_filter(&self) -> Result<U256> {
        self.rpc.new_pending_transaction_filter().await
    }

    async fn uninstall_filter(&self, filter_id: U256) -> Result<bool> {
        self.rpc.uninstall_filter(filter_id).await
    }
}

impl<N: NetworkSpec, R: ExecutionRpc<N>> ExecutionRpcClient<N, R> {
    async fn verify_proof_to_account(
        &self,
        proof: &EIP1186AccountProofResponse,
        block: &N::BlockResponse,
    ) -> Result<Account> {
        // Verify the account proof
        verify_account_proof(proof, block.header().state_root())?;
        // Verify the storage proofs, collecting the slot values
        let slot_map = verify_storage_proof(proof)?;
        // Verify the code hash
        let code = if proof.code_hash == KECCAK_EMPTY || proof.code_hash == B256::ZERO {
            Vec::new()
        } else {
            let code = self
                .rpc
                .get_code(proof.address, block.header().number().into())
                .await?;
            let code_hash = keccak256(&code);

            if proof.code_hash != code_hash {
                return Err(ExecutionError::CodeHashMismatch(
                    proof.address,
                    code_hash,
                    proof.code_hash,
                )
                .into());
            }

            code
        };

        Ok(Account {
            balance: proof.balance,
            nonce: proof.nonce,
            code_hash: proof.code_hash,
            code,
            storage_hash: proof.storage_hash,
            slots: slot_map,
        })
    }

    /// Verify the integrity of each log entry in the given array of logs by
    /// checking its inclusion in the corresponding transaction receipt
    /// and verifying the transaction receipt itself against the block's receipt root.
    async fn verify_logs(&self, logs: &[Log]) -> Result<()> {
        // Collect all (unique) block numbers
        let block_nums = logs
            .iter()
            .map(|log| {
                log.block_number
                    .ok_or_else(|| eyre::eyre!("block number not found in log"))
            })
            .collect::<Result<HashSet<_>, _>>()?;

        // Check if the number of blocks to prove is within the limit
        if block_nums.len() > MAX_SUPPORTED_BLOCKS_TO_PROVE_FOR_LOGS {
            return Err(ExecutionError::TooManyLogsToProve(
                logs.len(),
                block_nums.len(),
                MAX_SUPPORTED_BLOCKS_TO_PROVE_FOR_LOGS,
            )
            .into());
        }

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
