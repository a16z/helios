use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use ethers::prelude::{Address, U256};
use ethers::types::{Filter, Log, Transaction, TransactionReceipt, H256};
use eyre::{eyre, Result};

use common::errors::BlockNotFoundError;
use common::types::BlockTag;
use config::Config;
use consensus::rpc::nimbus_rpc::NimbusRpc;
use consensus::types::{ExecutionPayload, Header};
use consensus::ConsensusClient;
use execution::evm::Evm;
use execution::rpc::http_rpc::HttpRpc;
use execution::types::{CallOpts, ExecutionBlock};
use execution::ExecutionClient;

use crate::errors::NodeError;

pub struct Node {
    pub consensus: ConsensusClient<NimbusRpc>,
    pub execution: Arc<ExecutionClient<HttpRpc>>,
    pub config: Arc<Config>,
    payloads: BTreeMap<u64, ExecutionPayload>,
    finalized_payloads: BTreeMap<u64, ExecutionPayload>,
    pub history_size: usize,
}

impl Node {
    pub fn new(config: Arc<Config>) -> Result<Self, NodeError> {
        let consensus_rpc = &config.consensus_rpc;
        let checkpoint_hash = &config.checkpoint;
        let execution_rpc = &config.execution_rpc;

        let consensus = ConsensusClient::new(consensus_rpc, checkpoint_hash, config.clone())
            .map_err(NodeError::ConsensusClientCreationError)?;
        let execution = Arc::new(
            ExecutionClient::new(execution_rpc).map_err(NodeError::ExecutionClientCreationError)?,
        );

        let payloads = BTreeMap::new();
        let finalized_payloads = BTreeMap::new();

        Ok(Node {
            consensus,
            execution,
            config,
            payloads,
            finalized_payloads,
            history_size: 64,
        })
    }

    pub async fn sync(&mut self) -> Result<(), NodeError> {
        self.consensus
            .sync()
            .await
            .map_err(NodeError::ConsensusSyncError)?;
        self.update_payloads().await
    }

    pub async fn advance(&mut self) -> Result<(), NodeError> {
        self.consensus
            .advance()
            .await
            .map_err(NodeError::ConsensusAdvanceError)?;
        self.update_payloads().await
    }

    pub fn duration_until_next_update(&self) -> Duration {
        self.consensus
            .duration_until_next_update()
            .to_std()
            .unwrap()
    }

    async fn update_payloads(&mut self) -> Result<(), NodeError> {
        let latest_header = self.consensus.get_header();
        let latest_payload = self
            .consensus
            .get_execution_payload(&Some(latest_header.slot))
            .await
            .map_err(NodeError::ConsensusPayloadError)?;
        log::debug!("latest payload: {:?}", latest_payload.block_number);

        let finalized_header = self.consensus.get_finalized_header();
        let finalized_payload = self
            .consensus
            .get_execution_payload(&Some(finalized_header.slot))
            .await
            .map_err(NodeError::ConsensusPayloadError)?;
        log::debug!("finalized payload: {:?}", finalized_payload.block_number);

        self.payloads
            .insert(latest_payload.block_number, latest_payload);
        self.payloads
            .insert(finalized_payload.block_number, finalized_payload.clone());
        self.finalized_payloads
            .insert(finalized_payload.block_number, finalized_payload);

        while self.payloads.len() > self.history_size {
            log::debug!("Popping first payload...");
            self.payloads.pop_first();
        }

        // only save one finalized block per epoch
        // finality updates only occur on epoch boundries
        while self.finalized_payloads.len() > usize::max(self.history_size / 32, 1) {
            self.finalized_payloads.pop_first();
        }

        log::debug!("Successfully updated payloads");

        Ok(())
    }

    pub async fn call(&self, opts: &CallOpts, block: BlockTag) -> Result<Vec<u8>, NodeError> {
        self.check_blocktag_age(&block)?;

        let payload = self.get_payload(block)?;
        let mut evm = Evm::new(
            self.execution.clone(),
            payload,
            &self.payloads,
            self.chain_id(),
        );
        evm.call(opts).await.map_err(NodeError::ExecutionError)
    }

    pub async fn estimate_gas(&self, opts: &CallOpts) -> Result<u64, NodeError> {
        self.check_head_age()?;

        let payload = self.get_payload(BlockTag::Latest)?;
        let mut evm = Evm::new(
            self.execution.clone(),
            payload,
            &self.payloads,
            self.chain_id(),
        );
        evm.estimate_gas(opts)
            .await
            .map_err(NodeError::ExecutionError)
    }

    pub async fn get_balance(&self, address: &Address, block: BlockTag) -> Result<U256> {
        self.check_blocktag_age(&block)?;

        let payload = self.get_payload(block)?;
        let account = self.execution.get_account(address, None, payload).await?;
        Ok(account.balance)
    }

    pub async fn get_nonce(&self, address: &Address, block: BlockTag) -> Result<u64> {
        self.check_blocktag_age(&block)?;

        let payload = self.get_payload(block)?;
        let account = self.execution.get_account(address, None, payload).await?;
        Ok(account.nonce)
    }

    pub fn get_block_transaction_count_by_hash(&self, hash: &Vec<u8>) -> Result<u64> {
        let payload = self.get_payload_by_hash(hash)?;
        let transaction_count = payload.1.transactions.len();

        Ok(transaction_count as u64)
    }

    pub fn get_block_transaction_count_by_number(&self, block: BlockTag) -> Result<u64> {
        let payload = self.get_payload(block)?;
        let transaction_count = payload.transactions.len();

        Ok(transaction_count as u64)
    }

    pub async fn get_code(&self, address: &Address, block: BlockTag) -> Result<Vec<u8>> {
        self.check_blocktag_age(&block)?;

        let payload = self.get_payload(block)?;
        let account = self.execution.get_account(address, None, payload).await?;
        Ok(account.code)
    }

    pub async fn get_storage_at(
        &self,
        address: &Address,
        slot: H256,
        block: BlockTag,
    ) -> Result<U256> {
        self.check_head_age()?;

        let payload = self.get_payload(block)?;
        let account = self
            .execution
            .get_account(address, Some(&[slot]), payload)
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
        self.execution
            .get_transaction_receipt(tx_hash, &self.payloads)
            .await
    }

    pub async fn get_transaction_by_hash(&self, tx_hash: &H256) -> Result<Option<Transaction>> {
        self.execution
            .get_transaction(tx_hash, &self.payloads)
            .await
    }

    pub async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>> {
        self.execution.get_logs(filter, &self.payloads).await
    }

    // assumes tip of 1 gwei to prevent having to prove out every tx in the block
    pub fn get_gas_price(&self) -> Result<U256> {
        self.check_head_age()?;

        let payload = self.get_payload(BlockTag::Latest)?;
        let base_fee = U256::from_little_endian(&payload.base_fee_per_gas.to_bytes_le());
        let tip = U256::from(10_u64.pow(9));
        Ok(base_fee + tip)
    }

    // assumes tip of 1 gwei to prevent having to prove out every tx in the block
    pub fn get_priority_fee(&self) -> Result<U256> {
        let tip = U256::from(10_u64.pow(9));
        Ok(tip)
    }

    pub fn get_block_number(&self) -> Result<u64> {
        self.check_head_age()?;

        let payload = self.get_payload(BlockTag::Latest)?;
        Ok(payload.block_number)
    }

    pub async fn get_block_by_number(
        &self,
        block: BlockTag,
        full_tx: bool,
    ) -> Result<Option<ExecutionBlock>> {
        self.check_blocktag_age(&block)?;

        match self.get_payload(block) {
            Ok(payload) => self.execution.get_block(payload, full_tx).await.map(Some),
            Err(_) => Ok(None),
        }
    }

    pub async fn get_block_by_hash(
        &self,
        hash: &Vec<u8>,
        full_tx: bool,
    ) -> Result<Option<ExecutionBlock>> {
        let payload = self.get_payload_by_hash(hash);

        match payload {
            Ok(payload) => self.execution.get_block(payload.1, full_tx).await.map(Some),
            Err(_) => Ok(None),
        }
    }

    pub fn chain_id(&self) -> u64 {
        self.config.chain.chain_id
    }

    pub fn get_header(&self) -> Result<Header> {
        self.check_head_age()?;
        Ok(self.consensus.get_header().clone())
    }

    pub fn get_last_checkpoint(&self) -> Option<Vec<u8>> {
        self.consensus.last_checkpoint.clone()
    }

    fn get_payload(&self, block: BlockTag) -> Result<&ExecutionPayload, BlockNotFoundError> {
        match block {
            BlockTag::Latest => {
                let payload = self.payloads.last_key_value();
                Ok(payload.ok_or(BlockNotFoundError::new(BlockTag::Latest))?.1)
            }
            BlockTag::Finalized => {
                let payload = self.finalized_payloads.last_key_value();
                Ok(payload
                    .ok_or(BlockNotFoundError::new(BlockTag::Finalized))?
                    .1)
            }
            BlockTag::Number(num) => {
                let payload = self.payloads.get(&num);
                payload.ok_or(BlockNotFoundError::new(BlockTag::Number(num)))
            }
        }
    }

    fn get_payload_by_hash(&self, hash: &Vec<u8>) -> Result<(&u64, &ExecutionPayload)> {
        let payloads = self
            .payloads
            .iter()
            .filter(|entry| &entry.1.block_hash.to_vec() == hash)
            .collect::<Vec<(&u64, &ExecutionPayload)>>();

        payloads
            .get(0)
            .cloned()
            .ok_or(eyre!("Block not found by hash"))
    }

    fn check_head_age(&self) -> Result<(), NodeError> {
        let synced_slot = self.consensus.get_header().slot;
        let expected_slot = self.consensus.expected_current_slot();
        let slot_delay = expected_slot - synced_slot;

        if slot_delay > 10 {
            return Err(NodeError::OutOfSync(slot_delay));
        }

        Ok(())
    }

    fn check_blocktag_age(&self, block: &BlockTag) -> Result<(), NodeError> {
        match block {
            BlockTag::Latest => self.check_head_age(),
            BlockTag::Finalized => Ok(()),
            BlockTag::Number(_) => Ok(()),
        }
    }
}
