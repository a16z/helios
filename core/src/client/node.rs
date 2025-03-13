use std::sync::Arc;

use alloy::consensus::BlockHeader;
use alloy::network::BlockResponse;
use alloy::primitives::{Address, Bytes, B256, U256};
use alloy::rpc::types::{
    AccessListResult, EIP1186AccountProofResponse, Filter, FilterChanges, Log, SyncInfo, SyncStatus,
};
use eyre::{eyre, Result};

use helios_common::{
    execution_mode::ExecutionMode,
    fork_schedule::ForkSchedule,
    network_spec::NetworkSpec,
    types::{BlockTag, SubEventRx, SubscriptionType},
};
use helios_verifiable_api_client::http::HttpVerifiableApi;

use crate::consensus::Consensus;
use crate::errors::ClientError;
use crate::execution::client::ExecutionInnerClient;
use crate::execution::constants::MAX_STATE_HISTORY_LENGTH;
use crate::execution::evm::Evm;
use crate::execution::rpc::http_rpc::HttpRpc;
use crate::execution::spec::ExecutionSpec;
use crate::execution::state::{start_state_updater, State};
use crate::execution::ExecutionClient;
use crate::time::{SystemTime, UNIX_EPOCH};

pub struct Node<N: NetworkSpec, C: Consensus<N::BlockResponse>> {
    pub consensus: C,
    pub execution: Arc<ExecutionClient<N>>,
    fork_schedule: ForkSchedule,
}

impl<N: NetworkSpec, C: Consensus<N::BlockResponse>> Node<N, C> {
    pub fn new(
        execution_mode: ExecutionMode,
        mut consensus: C,
        fork_schedule: ForkSchedule,
    ) -> Result<Self, ClientError> {
        let block_recv = consensus.block_recv().unwrap();
        let finalized_block_recv = consensus.finalized_block_recv().unwrap();

        let state = State::new(MAX_STATE_HISTORY_LENGTH);
        let client_inner =
            ExecutionInnerClient::<N, HttpRpc<N>, HttpVerifiableApi>::make_inner_client(
                execution_mode,
                state.clone(),
            )
            .map_err(ClientError::InternalError)?;
        let execution = Arc::new(
            ExecutionClient::new(client_inner, state.clone(), fork_schedule)
                .map_err(ClientError::InternalError)?,
        );

        start_state_updater(state, execution.clone(), block_recv, finalized_block_recv);

        Ok(Node {
            consensus,
            execution,
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

    pub async fn estimate_gas(
        &self,
        tx: &N::TransactionRequest,
        block: BlockTag,
    ) -> Result<u64, ClientError> {
        self.check_blocktag_age(&block).await?;

        let mut evm = Evm::new(
            self.execution.clone(),
            self.chain_id(),
            self.fork_schedule,
            block,
        );

        evm.estimate_gas(tx).await.map_err(ClientError::EvmError)
    }

    pub async fn create_access_list(
        &self,
        tx: &N::TransactionRequest,
        block: BlockTag,
    ) -> Result<AccessListResult, ClientError> {
        self.check_blocktag_age(&block).await?;

        let mut evm = Evm::new(
            self.execution.clone(),
            self.chain_id(),
            self.fork_schedule,
            block,
        );

        let res = evm
            .create_access_list(tx, true)
            .await
            .map_err(ClientError::EvmError)?;

        Ok(res.access_list_result)
    }

    pub async fn get_balance(&self, address: Address, tag: BlockTag) -> Result<U256> {
        self.check_blocktag_age(&tag).await?;

        let account = self
            .execution
            .get_account(address, None, tag, false)
            .await?;
        Ok(account.account.balance)
    }

    pub async fn get_nonce(&self, address: Address, tag: BlockTag) -> Result<u64> {
        self.check_blocktag_age(&tag).await?;

        let account = self
            .execution
            .get_account(address, None, tag, false)
            .await?;
        Ok(account.account.nonce)
    }

    pub async fn get_block_transaction_count_by_hash(&self, hash: B256) -> Result<Option<u64>> {
        let block = self.execution.get_block(hash.into(), false).await?;
        Ok(block.map(|block| block.transactions().hashes().len() as u64))
    }

    pub async fn get_block_transaction_count_by_number(
        &self,
        tag: BlockTag,
    ) -> Result<Option<u64>> {
        let block = self.execution.get_block(tag.into(), false).await?;
        Ok(block.map(|block| block.transactions().hashes().len() as u64))
    }

    pub async fn get_code(&self, address: Address, tag: BlockTag) -> Result<Bytes> {
        self.check_blocktag_age(&tag).await?;

        let account = self.execution.get_account(address, None, tag, true).await?;
        account
            .code
            .ok_or(eyre!("Failed to fetch code for address"))
    }

    pub async fn get_storage_at(
        &self,
        address: Address,
        slot: U256,
        tag: BlockTag,
    ) -> Result<B256> {
        self.check_blocktag_age(&tag).await?;

        self.execution.get_storage_at(address, slot, tag).await
    }

    pub async fn get_proof(
        &self,
        address: Address,
        slots: Option<&[B256]>,
        block: BlockTag,
    ) -> Result<EIP1186AccountProofResponse> {
        self.check_blocktag_age(&block).await?;

        self.execution.get_proof(address, slots, block).await
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

        self.execution.get_block_receipts(block.into()).await
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
        let tag = BlockTag::Latest;

        let block = self
            .execution
            .get_block(tag.into(), false)
            .await?
            .ok_or(eyre!(ClientError::BlockNotFound(tag)))?;
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
        let tag = BlockTag::Latest;

        let block = self
            .execution
            .get_block(tag.into(), false)
            .await?
            .ok_or(eyre!(ClientError::BlockNotFound(tag)))?;
        Ok(U256::from(block.header().number()))
    }

    pub async fn get_block_by_number(
        &self,
        tag: BlockTag,
        full_tx: bool,
    ) -> Result<Option<N::BlockResponse>> {
        self.check_blocktag_age(&tag).await?;

        self.execution.get_block(tag.into(), full_tx).await
    }

    pub async fn get_block_by_hash(
        &self,
        hash: B256,
        full_tx: bool,
    ) -> Result<Option<N::BlockResponse>> {
        self.execution.get_block(hash.into(), full_tx).await
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
        let tag = BlockTag::Latest;

        let block = self
            .execution
            .get_block(tag.into(), false)
            .await?
            .ok_or(eyre!(ClientError::BlockNotFound(tag)))?;

        Ok(block.header().beneficiary())
    }

    async fn check_head_age(&self) -> Result<(), ClientError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| panic!("unreachable"))
            .as_secs();
        let tag = BlockTag::Latest;

        let block_timestamp = self
            .execution
            .get_block(tag.into(), false)
            .await
            .map_err(|_| ClientError::BlockNotFound(tag))?
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

    pub async fn subscribe(&self, sub_type: SubscriptionType) -> Result<SubEventRx<N>> {
        self.execution.subscribe(sub_type).await
    }
}
