use std::sync::Arc;

use ethers::prelude::{Address, U256};
use eyre::Result;

use config::Config;
use consensus::types::Header;
use consensus::ConsensusClient;
use execution::evm::Evm;
use execution::ExecutionClient;

pub struct Client {
    consensus: ConsensusClient,
    execution: ExecutionClient,
    config: Arc<Config>,
}

impl Client {
    pub async fn new(config: Arc<Config>) -> Result<Self> {
        let consensus_rpc = &config.general.consensus_rpc;
        let checkpoint_hash = &config.general.checkpoint;
        let execution_rpc = &config.general.execution_rpc;

        let consensus =
            ConsensusClient::new(consensus_rpc, checkpoint_hash, config.clone()).await?;
        let execution = ExecutionClient::new(execution_rpc);

        Ok(Client {
            consensus,
            execution,
            config,
        })
    }

    pub async fn sync(&mut self) -> Result<()> {
        self.consensus.sync().await
    }

    pub async fn call(&self, to: &Address, calldata: &Vec<u8>, value: U256) -> Result<Vec<u8>> {
        let payload = self.consensus.get_execution_payload().await?;
        let mut evm = Evm::new(self.execution.clone(), payload);
        evm.call(to, calldata, value)
    }

    pub async fn estimate_gas(&self, to: &Address, calldata: &Vec<u8>, value: U256) -> Result<u64> {
        let payload = self.consensus.get_execution_payload().await?;
        let mut evm = Evm::new(self.execution.clone(), payload);
        evm.estimate_gas(to, calldata, value)
    }

    pub async fn get_balance(&self, address: &Address) -> Result<U256> {
        let payload = self.consensus.get_execution_payload().await?;
        let account = self.execution.get_account(&address, None, &payload).await?;
        Ok(account.balance)
    }

    pub async fn get_nonce(&self, address: &Address) -> Result<U256> {
        let payload = self.consensus.get_execution_payload().await?;
        let account = self.execution.get_account(&address, None, &payload).await?;
        Ok(account.nonce)
    }

    pub async fn get_code(&self, address: &Address) -> Result<Vec<u8>> {
        let payload = self.consensus.get_execution_payload().await?;
        self.execution.get_code(&address, &payload).await
    }

    pub async fn get_storage_at(&self, address: &Address, slot: U256) -> Result<U256> {
        let payload = self.consensus.get_execution_payload().await?;
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

    pub async fn get_gas_price(&self) -> Result<U256> {
        let payload = self.consensus.get_execution_payload().await?;
        let base_fee = U256::from_little_endian(&payload.base_fee_per_gas.to_bytes_le());
        let tip = U256::from(10_u64.pow(9));
        Ok(base_fee + tip)
    }

    pub async fn get_priority_fee(&self) -> Result<U256> {
        let tip = U256::from(10_u64.pow(9));
        Ok(tip)
    }

    pub async fn get_block_number(&self) -> Result<u64> {
        let payload = self.consensus.get_execution_payload().await?;
        Ok(payload.block_number)
    }

    pub fn chain_id(&self) -> u64 {
        self.config.general.chain_id
    }

    pub fn get_header(&self) -> &Header {
        self.consensus.get_head()
    }
}
