use std::sync::Arc;

use alloy::primitives::{Address, Bytes, B256, U256};
use alloy::rpc::types::{Filter, Log, SyncInfo, SyncStatus};
use eyre::{eyre, Result};

use crate::consensus::Consensus;
use crate::errors::ClientError;
use crate::execution::evm::Evm;
use crate::execution::rpc::http_rpc::HttpRpc;
use crate::execution::state::State;
use crate::execution::ExecutionClient;
use crate::network_spec::NetworkSpec;
use crate::time::{SystemTime, UNIX_EPOCH};
use crate::types::{Block, BlockTag};

pub struct Node<N: NetworkSpec, C: Consensus<N::TransactionResponse>> {
    pub consensus: C,
    pub execution: Arc<ExecutionClient<N, HttpRpc<N>>>,
    pub history_size: usize,
}

impl<N: NetworkSpec, C: Consensus<N::TransactionResponse>> Node<N, C> {
    pub fn new(execution_rpc: &str, mut consensus: C) -> Result<Self, ClientError> {
        let block_recv = consensus.block_recv().take().unwrap();
        let finalized_block_recv = consensus.finalized_block_recv().take().unwrap();

        let state = State::new(block_recv, finalized_block_recv, 256, execution_rpc);
        let execution = Arc::new(
            ExecutionClient::new(execution_rpc, state).map_err(ClientError::InternalError)?,
        );

        Ok(Node {
            consensus,
            execution,
            history_size: 64,
        })
    }

    pub async fn call(
        &self,
        tx: &N::TransactionRequest,
        block: BlockTag,
    ) -> Result<Bytes, ClientError> {
        self.check_blocktag_age(&block).await?;

        let mut evm = Evm::new(self.execution.clone(), self.chain_id(), block);
        evm.call(tx).await.map_err(ClientError::EvmError)
    }

    pub async fn estimate_gas(&self, tx: &N::TransactionRequest) -> Result<u64, ClientError> {
        self.check_head_age().await?;

        let mut evm = Evm::new(self.execution.clone(), self.chain_id(), BlockTag::Latest);

        evm.estimate_gas(tx).await.map_err(ClientError::EvmError)
    }

    pub async fn get_balance(&self, address: Address, tag: BlockTag) -> Result<U256> {
        self.check_blocktag_age(&tag).await?;

        let account = self.execution.get_account(address, None, tag).await?;
        Ok(account.balance)
    }

    pub async fn get_nonce(&self, address: Address, tag: BlockTag) -> Result<u64> {
        self.check_blocktag_age(&tag).await?;

        let account = self.execution.get_account(address, None, tag).await?;
        Ok(account.nonce)
    }

    pub async fn get_block_transaction_count_by_hash(&self, hash: B256) -> Result<u64> {
        let block = self.execution.get_block_by_hash(hash, false).await?;
        let transaction_count = block.transactions.hashes().len();

        Ok(transaction_count as u64)
    }

    pub async fn get_block_transaction_count_by_number(&self, tag: BlockTag) -> Result<u64> {
        let block = self.execution.get_block(tag, false).await?;
        let transaction_count = block.transactions.hashes().len();

        Ok(transaction_count as u64)
    }

    pub async fn get_code(&self, address: Address, tag: BlockTag) -> Result<Bytes> {
        self.check_blocktag_age(&tag).await?;

        let account = self.execution.get_account(address, None, tag).await?;
        Ok(account.code.into())
    }

    pub async fn get_storage_at(
        &self,
        address: Address,
        slot: B256,
        tag: BlockTag,
    ) -> Result<U256> {
        self.check_head_age().await?;

        let account = self
            .execution
            .get_account(address, Some(&[slot]), tag)
            .await?;

        let value = account.slots.get(&slot);
        match value {
            Some(value) => Ok(*value),
            None => Err(eyre!("slot not found")),
        }
    }

    pub async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<B256> {
        self.execution.send_raw_transaction(bytes).await
    }

    pub async fn get_transaction_receipt(
        &self,
        tx_hash: B256,
    ) -> Result<Option<N::ReceiptResponse>> {
        self.execution.get_transaction_receipt(tx_hash).await
    }

    pub async fn get_transaction_by_hash(&self, tx_hash: B256) -> Option<N::TransactionResponse> {
        self.execution.get_transaction(tx_hash).await
    }

    pub async fn get_transaction_by_block_hash_and_index(
        &self,
        hash: B256,
        index: u64,
    ) -> Option<N::TransactionResponse> {
        self.execution
            .get_transaction_by_block_hash_and_index(hash, index)
            .await
    }

    pub async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>> {
        self.execution.get_logs(filter).await
    }

    pub async fn get_client_version(&self) -> Result<String> {
       self.execution.get_client_version().await
    }

    pub async fn get_filter_changes(&self, filter_id: U256) -> Result<Vec<Log>> {
        self.execution.get_filter_changes(filter_id).await
    }

    pub async fn uninstall_filter(&self, filter_id: U256) -> Result<bool> {
        self.execution.uninstall_filter(filter_id).await
    }

    pub async fn get_new_filter(&self, filter: &Filter) -> Result<U256> {
        self.execution.get_new_filter(filter).await
    }

    pub async fn get_new_block_filter(&self) -> Result<U256> {
        self.execution.get_new_block_filter().await
    }

    pub async fn get_new_pending_transaction_filter(&self) -> Result<U256> {
        self.execution.get_new_pending_transaction_filter().await
    }

    // assumes tip of 1 gwei to prevent having to prove out every tx in the block
    pub async fn get_gas_price(&self) -> Result<U256> {
        self.check_head_age().await?;

        let block = self.execution.get_block(BlockTag::Latest, false).await?;
        let base_fee = block.base_fee_per_gas;
        let tip = U256::from(10_u64.pow(9));
        Ok(base_fee + tip)
    }

    // assumes tip of 1 gwei to prevent having to prove out every tx in the block
    pub fn get_priority_fee(&self) -> Result<U256> {
        let tip = U256::from(10_u64.pow(9));
        Ok(tip)
    }

    pub async fn get_block_number(&self) -> Result<U256> {
        self.check_head_age().await?;

        let block = self.execution.get_block(BlockTag::Latest, false).await?;
        Ok(block.number.to())
    }

    pub async fn get_block_by_number(
        &self,
        tag: BlockTag,
        full_tx: bool,
    ) -> Result<Option<Block<N::TransactionResponse>>> {
        self.check_blocktag_age(&tag).await?;

        match self.execution.get_block(tag, full_tx).await {
            Ok(block) => Ok(Some(block)),
            Err(_) => Ok(None),
        }
    }

    pub async fn get_block_by_hash(
        &self,
        hash: B256,
        full_tx: bool,
    ) -> Result<Option<Block<N::TransactionResponse>>> {
        let block = self.execution.get_block_by_hash(hash, full_tx).await;

        match block {
            Ok(block) => Ok(Some(block)),
            Err(_) => Ok(None),
        }
    }

    pub fn chain_id(&self) -> u64 {
        self.consensus.chain_id()
    }

    pub async fn syncing(&self) -> Result<SyncStatus> {
        if self.check_head_age().await.is_ok() {
            Ok(SyncStatus::None)
        } else {
            let latest_synced_block = self.get_block_number().await.unwrap_or(U256::ZERO);
            let highest_block = self.consensus.expected_highest_block();

            Ok(SyncStatus::Info(Box::new(SyncInfo {
                current_block: latest_synced_block,
                highest_block: U256::from(highest_block),
                starting_block: U256::ZERO,
                ..Default::default()
            })))
        }
    }

    pub async fn get_coinbase(&self) -> Result<Address> {
        self.check_head_age().await?;

        let block = self.execution.get_block(BlockTag::Latest, false).await?;
        Ok(block.miner)
    }

    async fn check_head_age(&self) -> Result<(), ClientError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| panic!("unreachable"))
            .as_secs();

        let block_timestamp = self
            .execution
            .get_block(BlockTag::Latest, false)
            .await
            .map_err(|_| ClientError::OutOfSync(timestamp))?
            .timestamp
            .to();

        let delay = timestamp.checked_sub(block_timestamp).unwrap_or_default();
        if delay > 60 {
            return Err(ClientError::OutOfSync(delay));
        }

        Ok(())
    }

    async fn check_blocktag_age(&self, block: &BlockTag) -> Result<(), ClientError> {
        match block {
            BlockTag::Latest => self.check_head_age().await,
            BlockTag::Finalized => Ok(()),
            BlockTag::Number(_) => Ok(()),
        }
    }
}
