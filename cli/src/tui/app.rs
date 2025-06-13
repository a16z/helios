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
    pub parent_hash: B256,
    pub gas_used: u64,
    pub gas_limit: u64,
    pub base_fee: Option<u64>,
    pub transactions_count: usize,
}

#[derive(Clone, PartialEq)]
pub enum BlockType {
    Latest,
    Finalized,
}

pub struct App<N: NetworkSpec> {
    pub client: HeliosClient<N>,
    pub latest_block: Option<u64>,
    pub finalized_block: Option<u64>,
    pub safe_block: Option<u64>,
    pub sync_status: SyncStatus,
    pub chain_name: String,
    pub chain_id: u64,
    pub network_type: NetworkType,
    pub uptime: Instant,
    pub block_history: VecDeque<BlockInfo>,
}

#[derive(Clone, PartialEq)]
pub enum NetworkType {
    Ethereum,
    OpStack,
    Linea,
}

#[derive(Clone)]
pub enum SyncStatus {
    Synced,
    Syncing { current: u64, highest: u64 },
    Starting,
    Unknown,
}

impl<N: NetworkSpec> App<N> {
    pub fn new(client: HeliosClient<N>) -> Self {
        let chain_id = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(client.get_chain_id())
        });
        let (chain_name, network_type) = get_chain_info(chain_id);

        Self {
            client,
            latest_block: None,
            finalized_block: None,
            safe_block: None,
            sync_status: SyncStatus::Starting,
            chain_name,
            chain_id,
            network_type,
            uptime: Instant::now(),
            block_history: VecDeque::new(),
        }
    }

    pub async fn update(&mut self) {
        // Update latest block
        match self.client.get_block_number().await {
            Ok(block_number_u256) => {
                let block_number: u64 = block_number_u256.try_into().unwrap_or(0);

                // Always update latest_block
                self.latest_block = Some(block_number);

                // Check if this is a new block for history
                let should_add_to_history = self.block_history.is_empty()
                    || self
                        .block_history
                        .iter()
                        .all(|b| b.number != block_number || b.block_type != BlockType::Latest);

                if should_add_to_history && block_number > 0 {
                    // Get block timestamp
                    match self
                        .client
                        .get_block(BlockNumberOrTag::Latest.into(), false)
                        .await
                    {
                        Ok(Some(block)) => {
                            let header = block.header();
                            let timestamp = header.timestamp();

                            let block_info = BlockInfo {
                                number: block_number,
                                timestamp,
                                block_type: BlockType::Latest,
                                hash: header.hash(),
                                parent_hash: header.parent_hash(),
                                gas_used: header.gas_used(),
                                gas_limit: header.gas_limit(),
                                base_fee: header.base_fee_per_gas(),
                                transactions_count: block.transactions().len(),
                            };

                            self.add_block_to_history(block_info);
                        }
                        Ok(None) => {
                            // Block not found, client might not be ready
                        }
                        Err(_) => {
                            // Error getting block, client might not be ready
                        }
                    }
                }
            }
            Err(_) => {
                // Error getting block number, client might not be ready yet
            }
        }

        // Update finalized block (only for Ethereum)
        if self.network_type == NetworkType::Ethereum {
            if let Ok(Some(block)) = self
                .client
                .get_block(BlockNumberOrTag::Finalized.into(), false)
                .await
            {
                let block_number = block.header().number();

                // Always update finalized_block
                self.finalized_block = Some(block_number);

                // Check if this is a new block for history
                let should_add_to_history = self
                    .block_history
                    .iter()
                    .all(|b| b.number != block_number || b.block_type != BlockType::Finalized);

                if should_add_to_history {
                    let header = block.header();
                    let timestamp = header.timestamp();

                    let block_info = BlockInfo {
                        number: block_number,
                        timestamp,
                        block_type: BlockType::Finalized,
                        hash: header.hash(),
                        parent_hash: header.parent_hash(),
                        gas_used: header.gas_used(),
                        gas_limit: header.gas_limit(),
                        base_fee: header.base_fee_per_gas(),
                        transactions_count: block.transactions().len(),
                    };

                    self.add_block_to_history(block_info);
                }
            }
        }

        // Update safe block (only for Ethereum)
        if self.network_type == NetworkType::Ethereum {
            if let Ok(Some(block)) = self
                .client
                .get_block(BlockNumberOrTag::Safe.into(), false)
                .await
            {
                self.safe_block = Some(block.header().number());
            }
        }

        // Update sync status
        match self.client.syncing().await {
            Ok(alloy::rpc::types::SyncStatus::None) => {
                // Only mark as synced if we have a block number
                if self.latest_block.is_some() && self.latest_block.unwrap() > 0 {
                    self.sync_status = SyncStatus::Synced;
                } else {
                    self.sync_status = SyncStatus::Starting;
                }
            }
            Ok(alloy::rpc::types::SyncStatus::Info(info)) => {
                if let (Ok(current), Ok(highest)) = (
                    info.current_block.try_into() as Result<u64, _>,
                    info.highest_block.try_into() as Result<u64, _>,
                ) {
                    // For OpStack/Linea, highest_block is u64::MAX, so check differently
                    if highest == u64::MAX {
                        // For OpStack/Linea, check if we have blocks
                        if current > 0 {
                            self.sync_status = SyncStatus::Synced;
                        } else {
                            self.sync_status = SyncStatus::Starting;
                        }
                    } else if current >= highest && current > 0 {
                        self.sync_status = SyncStatus::Synced;
                    } else if current > 0 {
                        self.sync_status = SyncStatus::Syncing { current, highest };
                    } else {
                        self.sync_status = SyncStatus::Starting;
                    }
                } else {
                    self.sync_status = SyncStatus::Unknown;
                }
            }
            Err(_) => {
                // If we can't get sync status, base it on whether we have blocks
                if self.latest_block.is_some() && self.latest_block.unwrap() > 0 {
                    self.sync_status = SyncStatus::Unknown;
                } else {
                    self.sync_status = SyncStatus::Starting;
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

fn get_chain_info(chain_id: u64) -> (String, NetworkType) {
    match chain_id {
        1 => ("Ethereum Mainnet".to_string(), NetworkType::Ethereum),
        11155111 => ("Sepolia Testnet".to_string(), NetworkType::Ethereum),
        10 => ("OP Mainnet".to_string(), NetworkType::OpStack),
        11155420 => ("OP Sepolia".to_string(), NetworkType::OpStack),
        8453 => ("Base Mainnet".to_string(), NetworkType::OpStack),
        84532 => ("Base Sepolia".to_string(), NetworkType::OpStack),
        59144 => ("Linea Mainnet".to_string(), NetworkType::Linea),
        59141 => ("Linea Sepolia".to_string(), NetworkType::Linea),
        _ => (format!("Chain {}", chain_id), NetworkType::Ethereum),
    }
}
