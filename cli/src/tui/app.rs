use std::collections::VecDeque;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use alloy::consensus::BlockHeader;
use alloy::eips::BlockNumberOrTag;
use alloy::network::{primitives::HeaderResponse, BlockResponse};
use alloy::primitives::B256;

use helios_common::network_spec::NetworkSpec;
use helios_core::client::HeliosClient;

#[derive(Clone)]
pub struct BlockInfo {
    pub number: u64,
    pub timestamp: u64,
    pub block_type: BlockType,
    pub hash: B256,
    pub gas_used: u64,
    pub gas_limit: u64,
    pub base_fee: Option<u64>,
    pub transactions_count: usize,
}

#[derive(Clone, PartialEq)]
pub enum BlockType {
    Latest,
    Finalized,
    Safe,
}

pub struct App<N: NetworkSpec> {
    pub client: HeliosClient<N>,
    pub latest_block: Option<u64>,
    pub finalized_block: Option<u64>,
    pub safe_block: Option<u64>,
    pub sync_status: SyncStatus,
    pub chain_name: String,
    pub uptime: Instant,
    pub block_history: VecDeque<BlockInfo>,
}


#[derive(Clone)]
pub enum SyncStatus {
    Synced,
    Syncing,
}

impl<N: NetworkSpec> App<N> {
    pub fn new(client: HeliosClient<N>) -> Self {
        let chain_id = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(client.get_chain_id())
        });
        let chain_name = get_chain_name(chain_id);

        Self {
            client,
            latest_block: None,
            finalized_block: None,
            safe_block: None,
            sync_status: SyncStatus::Syncing,
            chain_name,
            uptime: Instant::now(),
            block_history: VecDeque::new(),
        }
    }

    pub async fn update(&mut self) {
        // Update latest block
        self.update_block_type(BlockNumberOrTag::Latest, BlockType::Latest)
            .await;

        // Update finalized block (if supported by the network)
        self.update_block_type(BlockNumberOrTag::Finalized, BlockType::Finalized)
            .await;

        // Update safe block (if supported by the network)
        self.update_block_type(BlockNumberOrTag::Safe, BlockType::Safe)
            .await;

        // Update sync status
        match self.client.syncing().await {
            Ok(alloy::rpc::types::SyncStatus::None) => {
                self.sync_status = SyncStatus::Synced;
            }
            Ok(alloy::rpc::types::SyncStatus::Info(_)) => {
                self.sync_status = SyncStatus::Syncing;
            }
            Err(_) => {
                // If we can't get sync status, assume syncing
                self.sync_status = SyncStatus::Syncing;
            }
        }
    }

    async fn update_block_type(&mut self, tag: BlockNumberOrTag, block_type: BlockType) {
        if let Ok(Some(block)) = self.client.get_block(tag.into(), false).await {
            let block_number = block.header().number();

            // Update the appropriate block number field
            match block_type {
                BlockType::Latest => self.latest_block = Some(block_number),
                BlockType::Finalized => self.finalized_block = Some(block_number),
                BlockType::Safe => self.safe_block = Some(block_number),
            }

            // Only add Latest and Finalized blocks to history (not Safe blocks)
            if matches!(block_type, BlockType::Latest | BlockType::Finalized) {
                // Check if this is a new block for history
                let should_add_to_history = self
                    .block_history
                    .iter()
                    .all(|b| b.number != block_number || b.block_type != block_type);

                if should_add_to_history {
                    let header = block.header();
                    let block_info = BlockInfo {
                        number: block_number,
                        timestamp: header.timestamp(),
                        block_type,
                        hash: header.hash(),
                        gas_used: header.gas_used(),
                        gas_limit: header.gas_limit(),
                        base_fee: header.base_fee_per_gas(),
                        transactions_count: block.transactions().len(),
                    };

                    self.add_block_to_history(block_info);
                }
            }
        }
    }

    fn add_block_to_history(&mut self, block: BlockInfo) {
        self.block_history.push_front(block);

        // Sort by block number descending (newest first)
        let mut sorted: Vec<BlockInfo> = self.block_history.drain(..).collect();
        sorted.sort_by(|a, b| b.number.cmp(&a.number));

        self.block_history = sorted.into_iter().collect();
    }

    pub fn get_uptime(&self) -> Duration {
        self.uptime.elapsed()
    }
}

impl BlockInfo {
    pub fn age(&self) -> Duration {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Duration::from_secs(current_time.saturating_sub(self.timestamp))
    }
}

fn get_chain_name(chain_id: u64) -> String {
    match chain_id {
        1 => "Ethereum Mainnet".to_string(),
        11155111 => "Sepolia Testnet".to_string(),
        10 => "OP Mainnet".to_string(),
        11155420 => "OP Sepolia".to_string(),
        8453 => "Base Mainnet".to_string(),
        84532 => "Base Sepolia".to_string(),
        59144 => "Linea Mainnet".to_string(),
        59141 => "Linea Sepolia".to_string(),
        _ => format!("Chain {}", chain_id),
    }
}
