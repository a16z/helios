use std::sync::Arc;
use std::time::Duration;

use ethers::prelude::{Address, U256};
use ethers::types::{
    Filter, Log, SyncProgress, SyncingStatus, Transaction, TransactionReceipt, H256,
};
use eyre::{eyre, Result};
use wasm_timer::{SystemTime, UNIX_EPOCH};

use common::types::{Block, BlockTag};
use config::Config;
use execution::state::State;

use consensus::database::FileDB;
use consensus::rpc::nimbus_rpc::NimbusRpc;
use consensus::ConsensusClient;
use execution::evm::Evm;
use execution::rpc::http_rpc::HttpRpc;
use execution::types::CallOpts;
use execution::ExecutionClient;

use crate::errors::NodeError;

pub struct Node {
    pub consensus: ConsensusClient<NimbusRpc, FileDB>,
    pub execution: Arc<ExecutionClient<HttpRpc>>,
    pub config: Arc<Config>,
    pub history_size: usize,
}

impl Node {
    pub fn new(config: Arc<Config>) -> Result<Self, NodeError> {
        let consensus_rpc = &config.consensus_rpc;
        let execution_rpc = &config.execution_rpc;

        let mut consensus = ConsensusClient::new(consensus_rpc, config.clone())
            .map_err(NodeError::ConsensusClientCreationError)?;

        let block_recv = consensus.block_recv.take().unwrap();
        let finalized_block_recv = consensus.finalized_block_recv.take().unwrap();

        let state = State::new(block_recv, finalized_block_recv, 256);
        let execution = Arc::new(
            ExecutionClient::new(execution_rpc, state)
                .map_err(NodeError::ExecutionClientCreationError)?,
        );

        Ok(Node {
            consensus,
            execution,
            config,
            history_size: 64,
        })
    }

    pub fn duration_until_next_update(&self) -> Duration {
        self.consensus
            .duration_until_next_update()
            .to_std()
            .unwrap()
    }

    pub async fn call(&self, opts: &CallOpts, block: BlockTag) -> Result<Vec<u8>, NodeError> {
        self.check_blocktag_age(&block).await?;

        let mut evm = Evm::new(self.execution.clone(), self.chain_id(), block);

        evm.call(opts).await.map_err(NodeError::ExecutionEvmError)
    }

    pub async fn estimate_gas(&self, opts: &CallOpts) -> Result<u64, NodeError> {
        self.check_head_age().await?;

        let mut evm = Evm::new(self.execution.clone(), self.chain_id(), BlockTag::Latest);

        evm.estimate_gas(opts)
            .await
            .map_err(NodeError::ExecutionEvmError)
    }

    pub async fn get_balance(&self, address: &Address, tag: BlockTag) -> Result<U256> {
        self.check_blocktag_age(&tag).await?;

        let account = self.execution.get_account(address, None, tag).await?;
        Ok(account.balance)
    }

    pub async fn get_nonce(&self, address: &Address, tag: BlockTag) -> Result<u64> {
        self.check_blocktag_age(&tag).await?;

        let account = self.execution.get_account(address, None, tag).await?;
        Ok(account.nonce)
    }

    pub async fn get_block_transaction_count_by_hash(&self, hash: &H256) -> Result<u64> {
        let block = self.execution.get_block_by_hash(*hash, false).await?;
        let transaction_count = block.transactions.hashes().len();

        Ok(transaction_count as u64)
    }

    pub async fn get_block_transaction_count_by_number(&self, tag: BlockTag) -> Result<u64> {
        let block = self.execution.get_block(tag, false).await?;
        let transaction_count = block.transactions.hashes().len();

        Ok(transaction_count as u64)
    }

    pub async fn get_code(&self, address: &Address, tag: BlockTag) -> Result<Vec<u8>> {
        self.check_blocktag_age(&tag).await?;

        let account = self.execution.get_account(address, None, tag).await?;
        Ok(account.code)
    }

    pub async fn get_storage_at(
        &self,
        address: &Address,
        slot: H256,
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

    pub async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<H256> {
        self.execution.send_raw_transaction(bytes).await
    }

    pub async fn get_transaction_receipt(
        &self,
        tx_hash: &H256,
    ) -> Result<Option<TransactionReceipt>> {
        self.execution.get_transaction_receipt(tx_hash).await
    }

    pub async fn get_transaction_by_hash(&self, tx_hash: &H256) -> Option<Transaction> {
        self.execution.get_transaction(*tx_hash).await
    }

    pub async fn get_transaction_by_block_hash_and_index(
        &self,
        hash: &H256,
        index: u64,
    ) -> Option<Transaction> {
        self.execution
            .get_transaction_by_block_hash_and_index(*hash, index)
            .await
    }

    pub async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>> {
        self.execution.get_logs(filter).await
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
        Ok(U256::from(block.number.as_u64()))
    }

    pub async fn get_block_by_number(&self, tag: BlockTag, full_tx: bool) -> Result<Option<Block>> {
        self.check_blocktag_age(&tag).await?;

        match self.execution.get_block(tag, full_tx).await {
            Ok(block) => Ok(Some(block)),
            Err(_) => Ok(None),
        }
    }

    pub async fn get_block_by_hash(&self, hash: &H256, full_tx: bool) -> Result<Option<Block>> {
        let block = self.execution.get_block_by_hash(*hash, full_tx).await;

        match block {
            Ok(block) => Ok(Some(block)),
            Err(_) => Ok(None),
        }
    }

    pub fn chain_id(&self) -> u64 {
        self.config.chain.chain_id
    }

    pub async fn syncing(&self) -> Result<SyncingStatus> {
        if self.check_head_age().await.is_ok() {
            Ok(SyncingStatus::IsFalse)
        } else {
            let latest_synced_block = self.get_block_number().await?;
            let highest_block = self.consensus.expected_current_slot();

            Ok(SyncingStatus::IsSyncing(Box::new(SyncProgress {
                current_block: latest_synced_block.as_u64().into(),
                highest_block: highest_block.into(),
                // TODO: use better start value
                starting_block: 0.into(),

                // these fields don't make sense for helios
                pulled_states: None,
                known_states: None,
                healed_bytecode_bytes: None,
                healed_bytecodes: None,
                healed_trienode_bytes: None,
                healed_trienodes: None,
                healing_bytecode: None,
                healing_trienodes: None,
                synced_account_bytes: None,
                synced_accounts: None,
                synced_bytecode_bytes: None,
                synced_bytecodes: None,
                synced_storage: None,
                synced_storage_bytes: None,
            })))
        }
    }

    pub async fn get_coinbase(&self) -> Result<Address> {
        self.check_head_age().await?;

        let block = self.execution.get_block(BlockTag::Latest, false).await?;
        Ok(block.miner)
    }

    async fn check_head_age(&self) -> Result<(), NodeError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let block_timestap = self
            .execution
            .get_block(BlockTag::Latest, false)
            .await
            .map_err(|_| NodeError::OutOfSync(timestamp))?
            .timestamp
            .as_u64();

        let delay = timestamp.checked_sub(block_timestap).unwrap_or_default();
        if delay > 60 {
            return Err(NodeError::OutOfSync(delay));
        }

        Ok(())
    }

    async fn check_blocktag_age(&self, block: &BlockTag) -> Result<(), NodeError> {
        match block {
            BlockTag::Latest => self.check_head_age().await,
            BlockTag::Finalized => Ok(()),
            BlockTag::Number(_) => Ok(()),
        }
    }
}
