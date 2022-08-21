use ethers::prelude::{Address, U256};
use eyre::Result;

use crate::{consensus::ConsensusClient, execution::ExecutionClient, consensus_rpc::Header};

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

    pub async fn get_balance(&mut self, address: Address) -> Result<U256> {
        let payload = self.consensus.get_execution_payload().await?;
        let account = self.execution.get_account(&address, &payload).await?;
        Ok(account.balance)
    }

    pub async fn get_nonce(&mut self, address: Address) -> Result<U256> {
        let payload = self.consensus.get_execution_payload().await?;
        let account = self.execution.get_account(&address, &payload).await?;
        Ok(account.nonce)
    }

    pub fn get_header(&self) -> &Header {
        self.consensus.get_head()
    }
}
