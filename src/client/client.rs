use ethers::prelude::{Address, U256};
use eyre::Result;

use crate::consensus::types::Header;
use crate::consensus::ConsensusClient;
use crate::execution::ExecutionClient;

pub struct Client {
    consensus: ConsensusClient,
    execution: ExecutionClient,
}

impl Client {
    pub async fn new(
        consensus_rpc: &str,
        execution_rpc: &str,
        checkpoint_hash: &str,
    ) -> Result<Self> {
        let consensus = ConsensusClient::new(consensus_rpc, checkpoint_hash).await?;
        let execution = ExecutionClient::new(execution_rpc);

        Ok(Client {
            consensus,
            execution,
        })
    }

    pub async fn sync(&mut self) -> Result<()> {
        self.consensus.sync().await
    }

    pub async fn get_balance(&mut self, address: &Address) -> Result<U256> {
        let payload = self.consensus.get_execution_payload().await?;
        let account = self.execution.get_account(&address, None, &payload).await?;
        Ok(account.balance)
    }

    pub async fn get_nonce(&mut self, address: &Address) -> Result<U256> {
        let payload = self.consensus.get_execution_payload().await?;
        let account = self.execution.get_account(&address, None, &payload).await?;
        Ok(account.nonce)
    }

    pub async fn get_code(&mut self, address: &Address) -> Result<Vec<u8>> {
        let payload = self.consensus.get_execution_payload().await?;
        self.execution.get_code(&address, &payload).await
    }

    pub async fn get_storage_at(&mut self, address: &Address, slot: U256) -> Result<U256> {
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

    pub fn get_header(&self) -> &Header {
        self.consensus.get_head()
    }
}
