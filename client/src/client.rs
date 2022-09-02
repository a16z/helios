use std::collections::HashMap;
use std::sync::Arc;

use ethers::prelude::{Address, U256};
use ethers::types::TransactionReceipt;
use eyre::Result;

use config::Config;
use consensus::types::{ExecutionPayload, Header};
use consensus::ConsensusClient;
use execution::evm::Evm;
use execution::types::{CallOpts, ExecutionBlock};
use execution::ExecutionClient;

pub struct Client {
    consensus: ConsensusClient,
    execution: ExecutionClient,
    config: Arc<Config>,
    payloads: HashMap<u64, ExecutionPayload>,
    block_head: u64,
}

impl Client {
    pub async fn new(config: Arc<Config>) -> Result<Self> {
        let consensus_rpc = &config.general.consensus_rpc;
        let checkpoint_hash = &config.general.checkpoint;
        let execution_rpc = &config.general.execution_rpc;

        let consensus =
            ConsensusClient::new(consensus_rpc, checkpoint_hash, config.clone()).await?;
        let execution = ExecutionClient::new(execution_rpc);

        let payloads = HashMap::new();

        Ok(Client {
            consensus,
            execution,
            config,
            payloads,
            block_head: 0,
        })
    }

    pub async fn sync(&mut self) -> Result<()> {
        self.consensus.sync().await?;

        let head = self.consensus.get_head();
        let payload = self
            .consensus
            .get_execution_payload(&Some(head.slot))
            .await?;
        self.block_head = payload.block_number;
        self.payloads.insert(payload.block_number, payload);

        Ok(())
    }

    pub async fn advance(&mut self) -> Result<()> {
        self.consensus.advance().await?;

        let head = self.consensus.get_head();
        let payload = self
            .consensus
            .get_execution_payload(&Some(head.slot))
            .await?;
        self.block_head = payload.block_number;
        self.payloads.insert(payload.block_number, payload);

        Ok(())
    }

    pub fn call(&self, opts: &CallOpts, block: &Option<u64>) -> Result<Vec<u8>> {
        let payload = self.get_payload(block)?;
        let mut evm = Evm::new(self.execution.clone(), payload);
        evm.call(opts)
    }

    pub fn estimate_gas(&self, opts: &CallOpts) -> Result<u64> {
        let payload = self.get_payload(&None)?;
        let mut evm = Evm::new(self.execution.clone(), payload);
        evm.estimate_gas(opts)
    }

    pub async fn get_balance(&self, address: &Address, block: &Option<u64>) -> Result<U256> {
        let payload = self.get_payload(block)?;
        let account = self.execution.get_account(&address, None, &payload).await?;
        Ok(account.balance)
    }

    pub async fn get_nonce(&self, address: &Address, block: &Option<u64>) -> Result<U256> {
        let payload = self.get_payload(block)?;
        let account = self.execution.get_account(&address, None, &payload).await?;
        Ok(account.nonce)
    }

    pub async fn get_code(&self, address: &Address, block: &Option<u64>) -> Result<Vec<u8>> {
        let payload = self.get_payload(block)?;
        self.execution.get_code(&address, &payload).await
    }

    pub async fn get_storage_at(&self, address: &Address, slot: U256) -> Result<U256> {
        let payload = self.get_payload(&None)?;
        let account = self
            .execution
            .get_account(address, Some(&[slot]), &payload)
            .await?;
        let value = account.slots.get(&slot);
        match value {
            Some(value) => Ok(*value),
            None => Err(eyre::eyre!("Slot Not Found")),
        }
    }

    pub async fn send_raw_transaction(&self, bytes: &Vec<u8>) -> Result<Vec<u8>> {
        self.execution.send_raw_transaction(bytes).await
    }

    pub async fn get_transaction_receipt(
        &self,
        tx_hash: &Vec<u8>,
    ) -> Result<Option<TransactionReceipt>> {
        self.execution
            .get_transaction_receipt(tx_hash, &self.payloads)
            .await
    }

    pub fn get_gas_price(&self) -> Result<U256> {
        let payload = self.get_payload(&None)?;
        let base_fee = U256::from_little_endian(&payload.base_fee_per_gas.to_bytes_le());
        let tip = U256::from(10_u64.pow(9));
        Ok(base_fee + tip)
    }

    pub fn get_priority_fee(&self) -> Result<U256> {
        let tip = U256::from(10_u64.pow(9));
        Ok(tip)
    }

    pub fn get_block_number(&self) -> Result<u64> {
        let payload = self.get_payload(&None)?;
        Ok(payload.block_number)
    }

    pub fn get_block_by_number(&self, block: &Option<u64>) -> Result<ExecutionBlock> {
        let payload = self.get_payload(block)?;
        self.execution.get_block(&payload)
    }

    pub fn chain_id(&self) -> u64 {
        self.config.general.chain_id
    }

    pub fn get_header(&self) -> &Header {
        self.consensus.get_head()
    }

    fn get_payload(&self, block: &Option<u64>) -> Result<ExecutionPayload> {
        match block {
            Some(block) => {
                let payload = self.payloads.get(block);
                match payload {
                    Some(payload) => Ok(payload.clone()),
                    None => Err(eyre::eyre!("Block Not Found")),
                }
            }
            None => {
                let payload = self.payloads.get(&self.block_head);
                match payload {
                    Some(payload) => Ok(payload.clone()),
                    None => Err(eyre::eyre!("Block Not Found")),
                }
            }
        }
    }
}
