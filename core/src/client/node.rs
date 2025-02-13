use std::sync::Arc;

use alloy::consensus::BlockHeader;
use alloy::network::BlockResponse;
use alloy::primitives::{Address, Bytes, B256, U256};
use alloy::rpc::types::serde_helpers::JsonStorageKey;
use alloy::rpc::types::{Filter, FilterChanges, Log, SyncInfo, SyncStatus};
use eyre::{eyre, Result};

use helios_common::{fork_schedule::ForkSchedule, network_spec::NetworkSpec, types::BlockTag};
use helios_verifiable_api_client::VerifiableApiClient;

use crate::consensus::Consensus;
use crate::errors::ClientError;
use crate::execution::evm::Evm;
use crate::execution::rpc::http_rpc::HttpRpc;
use crate::execution::state::State;
use crate::execution::ExecutionClient;
use crate::time::{SystemTime, UNIX_EPOCH};

pub struct Node<N: NetworkSpec, C: Consensus<N::BlockResponse>> {
    pub consensus: C,
    pub execution: Arc<ExecutionClient<N, HttpRpc<N>, VerifiableApiClient>>,
    pub history_size: usize,
    fork_schedule: ForkSchedule,
}

impl<N: NetworkSpec, C: Consensus<N::BlockResponse>> Node<N, C> {
    pub fn new(
        execution_rpc: &str,
        verifiable_api: Option<&str>,
        mut consensus: C,
        fork_schedule: ForkSchedule,
    ) -> Result<Self, ClientError> {
        let block_recv = consensus.block_recv().take().unwrap();
        let finalized_block_recv = consensus.finalized_block_recv().take().unwrap();

        let state = State::new(block_recv, finalized_block_recv, 256, execution_rpc);
        let execution = Arc::new(
            ExecutionClient::new(execution_rpc, verifiable_api, state, fork_schedule)
                .map_err(ClientError::InternalError)?,
        );

        Ok(Node {
            consensus,
            execution,
            history_size: 64,
            fork_schedule,
        })
    }

    pub async fn call(
        &self,
        tx: &N::TransactionRequest,
        block: BlockTag,
    ) -> Result<Bytes, ClientError> {
        self.check_blocktag_age(&block).await?;

        let mut evm = Evm::new(
            self.execution.clone(),
            self.chain_id(),
            self.fork_schedule,
            block,
        );
        evm.call(tx).await.map_err(ClientError::EvmError)
    }

    pub async fn estimate_gas(&self, tx: &N::TransactionRequest) -> Result<u64, ClientError> {
        self.check_head_age().await?;

        let mut evm = Evm::new(
            self.execution.clone(),
            self.chain_id(),
            self.fork_schedule,
            BlockTag::Latest,
        );

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

    pub async fn get_block_transaction_count_by_hash(&self, hash: B256) -> Result<Option<u64>> {
        let block = self.execution.get_block_by_hash(hash, false).await;
        Ok(block.map(|block| block.transactions().hashes().len() as u64))
    }

    pub async fn get_block_transaction_count_by_number(
        &self,
        tag: BlockTag,
    ) -> Result<Option<u64>> {
        let block = self.execution.get_block(tag, false).await;
        Ok(block.map(|block| block.transactions().hashes().len() as u64))
    }

    pub async fn get_code(&self, address: Address, tag: BlockTag) -> Result<Bytes> {
        self.check_blocktag_age(&tag).await?;

        let account = self.execution.get_account(address, None, tag).await?;
        Ok(account.code.into())
    }

    pub async fn get_storage_at(
        &self,
        address: Address,
        slot: JsonStorageKey,
        tag: BlockTag,
    ) -> Result<B256> {
        self.check_blocktag_age(&tag).await?;

        self.execution.get_storage_at(address, slot, tag).await
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

    pub async fn get_block_receipts(
        &self,
        block: BlockTag,
    ) -> Result<Option<Vec<N::ReceiptResponse>>> {
        self.check_blocktag_age(&block).await?;

        self.execution.get_block_receipts(block).await
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

    pub async fn get_transaction_by_block_number_and_index(
        &self,
        block: BlockTag,
        index: u64,
    ) -> Result<Option<N::TransactionResponse>> {
        self.check_blocktag_age(&block).await?;

        Ok(self
            .execution
            .get_transaction_by_block_number_and_index(block, index)
            .await)
    }

    pub async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>> {
        self.execution.get_logs(filter).await
    }

    pub async fn client_version(&self) -> String {
        let helios_version = std::env!("CARGO_PKG_VERSION");
        format!("helios-{}", helios_version)
    }

    pub async fn get_filter_changes(&self, filter_id: U256) -> Result<FilterChanges> {
        self.execution.get_filter_changes(filter_id).await
    }

    pub async fn get_filter_logs(&self, filter_id: U256) -> Result<Vec<Log>> {
        self.execution.get_filter_logs(filter_id).await
    }

    pub async fn uninstall_filter(&self, filter_id: U256) -> Result<bool> {
        self.execution.uninstall_filter(filter_id).await
    }

    pub async fn new_filter(&self, filter: &Filter) -> Result<U256> {
        self.execution.new_filter(filter).await
    }

    pub async fn new_block_filter(&self) -> Result<U256> {
        self.execution.new_block_filter().await
    }

    pub async fn new_pending_transaction_filter(&self) -> Result<U256> {
        self.execution.new_pending_transaction_filter().await
    }

    // assumes tip of 1 gwei to prevent having to prove out every tx in the block
    pub async fn get_gas_price(&self) -> Result<U256> {
        self.check_head_age().await?;

        let block = self
            .execution
            .get_block(BlockTag::Latest, false)
            .await
            .ok_or(eyre!(ClientError::BlockNotFound(BlockTag::Latest)))?;
        let base_fee = block.header().base_fee_per_gas().unwrap_or(0_u64);
        let tip = 10_u64.pow(9);
        Ok(U256::from(base_fee + tip))
    }

    pub async fn blob_base_fee(&self, block: BlockTag) -> Result<U256> {
        Ok(self.execution.blob_base_fee(block).await)
    }

    // assumes tip of 1 gwei to prevent having to prove out every tx in the block
    pub fn get_priority_fee(&self) -> Result<U256> {
        let tip = U256::from(10_u64.pow(9));
        Ok(tip)
    }

    pub async fn get_block_number(&self) -> Result<U256> {
        self.check_head_age().await?;

        let block = self
            .execution
            .get_block(BlockTag::Latest, false)
            .await
            .ok_or(eyre!(ClientError::BlockNotFound(BlockTag::Latest)))?;
        Ok(U256::from(block.header().number()))
    }

    pub async fn get_block_by_number(
        &self,
        tag: BlockTag,
        full_tx: bool,
    ) -> Result<Option<N::BlockResponse>> {
        self.check_blocktag_age(&tag).await?;

        let block = self.execution.get_block(tag, full_tx).await;
        Ok(block)
    }

    pub async fn get_block_by_hash(
        &self,
        hash: B256,
        full_tx: bool,
    ) -> Result<Option<N::BlockResponse>> {
        let block = self.execution.get_block_by_hash(hash, full_tx).await;
        Ok(block)
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

        let block = self
            .execution
            .get_block(BlockTag::Latest, false)
            .await
            .ok_or(eyre!(ClientError::BlockNotFound(BlockTag::Latest)))?;

        Ok(block.header().beneficiary())
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
            .ok_or_else(|| ClientError::OutOfSync(timestamp))?
            .header()
            .timestamp();

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
