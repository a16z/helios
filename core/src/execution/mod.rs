use std::collections::HashMap;
use std::marker::PhantomData;

use alloy::consensus::BlockHeader;
use alloy::eips::BlockId;
use alloy::network::primitives::HeaderResponse;
use alloy::network::BlockResponse;
use alloy::primitives::{Address, B256, U256};
use alloy::rpc::types::serde_helpers::JsonStorageKey;
use alloy::rpc::types::{BlockTransactions, Filter, FilterChanges, Log};
use eyre::Result;
use helios_verifiable_api_client::VerifiableApi;
use proof::{ensure_logs_match_filter, verify_block_receipts};
use revm::primitives::BlobExcessGasAndPrice;
use tracing::{info, warn};

use helios_common::{
    fork_schedule::ForkSchedule,
    network_spec::NetworkSpec,
    types::{Account, BlockTag},
};

use self::errors::ExecutionError;
use self::rpc::ExecutionRpc;
use self::state::{FilterType, State};
use self::verified_client::{
    api::VerifiableMethodsApi, rpc::VerifiableMethodsRpc, VerifiableMethods,
    VerifiableMethodsClient,
};

pub mod constants;
pub mod errors;
pub mod evm;
pub mod proof;
pub mod rpc;
pub mod state;
pub mod verified_client;

#[derive(Clone)]
pub struct ExecutionClient<N: NetworkSpec, R: ExecutionRpc<N>, A: VerifiableApi<N>> {
    pub rpc: R,
    verified_methods: VerifiableMethodsClient<N, R, A>,
    state: State<N, R>,
    fork_schedule: ForkSchedule,
    _marker: PhantomData<A>,
}

impl<N: NetworkSpec, R: ExecutionRpc<N>, A: VerifiableApi<N>> ExecutionClient<N, R, A> {
    pub fn new(
        rpc: &str,
        verifiable_api: Option<&str>,
        state: State<N, R>,
        fork_schedule: ForkSchedule,
    ) -> Result<Self> {
        let verified_methods = if let Some(verifiable_api) = verifiable_api {
            info!(target: "helios::execution", "using Verifiable-API url={}", verifiable_api);
            VerifiableMethodsClient::Api(VerifiableMethodsApi::new(verifiable_api, state.clone())?)
        } else {
            info!(target: "helios::execution", "Using JSON-RPC url={}", rpc);
            VerifiableMethodsClient::Rpc(VerifiableMethodsRpc::new(rpc, state.clone())?)
        };
        let rpc: R = ExecutionRpc::new(rpc)?;
        Ok(Self {
            rpc,
            verified_methods,
            state,
            fork_schedule,
            _marker: PhantomData,
        })
    }

    pub async fn check_rpc(&self, chain_id: u64) -> Result<()> {
        if self.rpc.chain_id().await? != chain_id {
            Err(ExecutionError::IncorrectRpcNetwork().into())
        } else {
            Ok(())
        }
    }

    pub async fn get_account(
        &self,
        address: Address,
        slots: Option<&[B256]>,
        tag: BlockTag,
    ) -> Result<Account> {
        self.verified_methods
            .client()
            .get_account(address, slots, tag)
            .await
    }

    pub async fn get_storage_at(
        &self,
        address: Address,
        slot: JsonStorageKey,
        block: BlockTag,
    ) -> Result<B256> {
        let storage_key = slot.as_b256();

        let account = self
            .get_account(address, Some(&[storage_key]), block)
            .await?;

        let value = account.slots.get(&storage_key);
        match value {
            Some(value) => Ok((*value).into()),
            None => Err(ExecutionError::InvalidStorageProof(address, storage_key).into()),
        }
    }

    pub async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<B256> {
        self.rpc.send_raw_transaction(bytes).await
    }

    pub async fn get_block(&self, tag: BlockTag, full_tx: bool) -> Option<N::BlockResponse> {
        let block = self.state.get_block(tag).await;
        if block.is_none() {
            warn!(target: "helios::execution", "requested block not found in state: {}", tag);
            return None;
        }
        let mut block = block.unwrap();

        if !full_tx {
            *block.transactions_mut() =
                BlockTransactions::Hashes(block.transactions().hashes().collect());
        }

        Some(block)
    }

    pub async fn blob_base_fee(&self, block: BlockTag) -> U256 {
        let block = self.state.get_block(block).await;
        let Some(block) = block else {
            warn!(target: "helios::execution", "requested block not found");
            return U256::from(0);
        };

        let parent_hash = block.header().parent_hash();
        let parent_block = self.get_block_by_hash(parent_hash, false).await;
        if parent_block.is_none() {
            warn!(target: "helios::execution", "requested parent block not foundß");
            return U256::from(0);
        };

        let excess_blob_gas = parent_block.unwrap().header().excess_blob_gas().unwrap();
        let is_prague = block.header().timestamp() >= self.fork_schedule.prague_timestamp;
        U256::from(BlobExcessGasAndPrice::new(excess_blob_gas, is_prague).blob_gasprice)
    }

    pub async fn get_block_by_hash(&self, hash: B256, full_tx: bool) -> Option<N::BlockResponse> {
        let block = self.state.get_block_by_hash(hash).await;
        if block.is_none() {
            warn!(target: "helios::execution", "requested block not found in state: {}", hash);
            return None;
        }
        let mut block = block.unwrap();

        if !full_tx {
            *block.transactions_mut() =
                BlockTransactions::Hashes(block.transactions().hashes().collect());
        }

        Some(block)
    }

    pub async fn get_transaction_by_block_hash_and_index(
        &self,
        block_hash: B256,
        index: u64,
    ) -> Option<N::TransactionResponse> {
        self.state
            .get_transaction_by_block_hash_and_index(block_hash, index)
            .await
    }

    pub async fn get_transaction_by_block_number_and_index(
        &self,
        tag: BlockTag,
        index: u64,
    ) -> Option<N::TransactionResponse> {
        self.state
            .get_transaction_by_block_and_index(tag, index)
            .await
    }

    pub async fn get_transaction_receipt(
        &self,
        tx_hash: B256,
    ) -> Result<Option<N::ReceiptResponse>> {
        self.verified_methods
            .client()
            .get_transaction_receipt(tx_hash)
            .await
    }

    pub async fn get_block_receipts(
        &self,
        tag: BlockTag,
    ) -> Result<Option<Vec<N::ReceiptResponse>>> {
        let block = self.state.get_block(tag).await;
        let block = if let Some(block) = block {
            block
        } else {
            return Ok(None);
        };
        let block_id = BlockId::from(block.header().number());

        let receipts = self
            .rpc
            .get_block_receipts(block_id)
            .await?
            .ok_or(eyre::eyre!(ExecutionError::NoReceiptsForBlock(tag)))?;

        verify_block_receipts::<N>(&receipts, &block)?;

        Ok(Some(receipts))
    }

    pub async fn get_transaction(&self, hash: B256) -> Option<N::TransactionResponse> {
        self.state.get_transaction(hash).await
    }

    pub async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>> {
        let filter = filter.clone();

        // avoid fetching logs for a block helios hasn't seen yet
        let filter = if filter.get_to_block().is_none() && filter.get_block_hash().is_none() {
            let block = self.state.latest_block_number().await.unwrap();
            let filter = filter.to_block(block);
            if filter.get_from_block().is_none() {
                filter.from_block(block)
            } else {
                filter
            }
        } else {
            filter
        };

        let logs = self.verified_methods.client().get_logs(&filter).await?;
        ensure_logs_match_filter(&logs, &filter)?;
        Ok(logs)
    }

    pub async fn get_filter_changes(&self, filter_id: U256) -> Result<FilterChanges> {
        let filter_type = self.state.get_filter(&filter_id).await;

        Ok(match &filter_type {
            None => {
                // only concerned with filters created via helios
                return Err(ExecutionError::FilterNotFound(filter_id).into());
            }
            Some(FilterType::Logs(filter)) => {
                // underlying RPC takes care of keeping track of changes
                let filter_changes = self
                    .verified_methods
                    .client()
                    .get_filter_changes(filter_id)
                    .await?;
                let logs = filter_changes.as_logs().unwrap_or(&[]);
                ensure_logs_match_filter(logs, filter)?;
                FilterChanges::Logs(logs.to_vec())
            }
            Some(FilterType::NewBlock(last_block_num)) => {
                let blocks = self
                    .state
                    .get_blocks_after(BlockTag::Number(*last_block_num))
                    .await;
                if !blocks.is_empty() {
                    // keep track of the last block number in state
                    // so next call can filter starting from the prev call's (last block number + 1)
                    self.state
                        .push_filter(
                            filter_id,
                            FilterType::NewBlock(blocks.last().unwrap().header().number()),
                        )
                        .await;
                }
                let block_hashes = blocks.into_iter().map(|b| b.header().hash()).collect();
                FilterChanges::Hashes(block_hashes)
            }
            Some(FilterType::PendingTransactions) => {
                // underlying RPC takes care of keeping track of changes
                let filter_changes = self.rpc.get_filter_changes(filter_id).await?;
                let tx_hashes = filter_changes.as_hashes().unwrap_or(&[]);
                FilterChanges::Hashes(tx_hashes.to_vec())
            }
        })
    }

    pub async fn get_filter_logs(&self, filter_id: U256) -> Result<Vec<Log>> {
        let filter_type = self.state.get_filter(&filter_id).await;

        match &filter_type {
            Some(FilterType::Logs(filter)) => {
                let logs = self
                    .verified_methods
                    .client()
                    .get_filter_logs(filter_id)
                    .await?;
                ensure_logs_match_filter(&logs, filter)?;
                Ok(logs)
            }
            _ => {
                // only concerned with filters created via helios
                return Err(ExecutionError::FilterNotFound(filter_id).into());
            }
        }
    }

    pub async fn uninstall_filter(&self, filter_id: U256) -> Result<bool> {
        // remove the filter from the state
        self.state.remove_filter(&filter_id).await;
        self.rpc.uninstall_filter(filter_id).await
    }

    pub async fn new_filter(&self, filter: &Filter) -> Result<U256> {
        let filter = filter.clone();

        // avoid submitting a filter for logs for a block helios hasn't seen yet
        let filter = if filter.get_to_block().is_none() && filter.get_block_hash().is_none() {
            let block = self.state.latest_block_number().await.unwrap();
            let filter = filter.to_block(block);
            if filter.get_from_block().is_none() {
                filter.from_block(block)
            } else {
                filter
            }
        } else {
            filter
        };
        let filter_id = self.rpc.new_filter(&filter).await?;

        // record the filter in the state
        self.state
            .push_filter(filter_id, FilterType::Logs(filter))
            .await;

        Ok(filter_id)
    }

    pub async fn new_block_filter(&self) -> Result<U256> {
        let filter_id = self.rpc.new_block_filter().await?;

        // record the filter in the state
        let latest_block_num = self.state.latest_block_number().await.unwrap_or(1);
        self.state
            .push_filter(filter_id, FilterType::NewBlock(latest_block_num))
            .await;

        Ok(filter_id)
    }

    pub async fn new_pending_transaction_filter(&self) -> Result<U256> {
        let filter_id = self.rpc.new_pending_transaction_filter().await?;

        // record the filter in the state
        self.state
            .push_filter(filter_id, FilterType::PendingTransactions)
            .await;

        Ok(filter_id)
    }

    pub async fn create_access_list(
        &self,
        tx: &N::TransactionRequest,
        block: Option<BlockId>,
    ) -> Result<HashMap<Address, Account>> {
        self.verified_methods
            .client()
            .create_access_list(tx, block)
            .await
    }
}
