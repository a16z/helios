use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

use alloy::consensus::BlockHeader;
use alloy::eips::{BlockId, BlockNumberOrTag};
use alloy::network::BlockResponse;
use alloy::primitives::{Address, Bytes, B256, U256};
use alloy::rpc::types::{
    AccessListResult, EIP1186AccountProofResponse, Filter, Log, SyncInfo, SyncStatus,
};
use async_trait::async_trait;
use eyre::{eyre, Result};
use revm::context_interface::block::BlobExcessGasAndPrice;
use tokio::time::interval;
use tokio::{select, sync::broadcast::Sender};
use tracing::{info, warn};

use helios_common::{
    fork_schedule::ForkSchedule,
    network_spec::NetworkSpec,
    types::{BlockTag, SubEventRx, SubscriptionEvent, SubscriptionType},
};

use crate::consensus::Consensus;
use crate::errors::ClientError;
use crate::execution::evm::Evm;
use crate::execution::filter_state::{FilterState, FilterType};
use crate::execution::providers::ExecutionProivder;
use crate::time::{SystemTime, UNIX_EPOCH};

use super::api::HeliosApi;

pub struct Node<N: NetworkSpec, C: Consensus<N::BlockResponse>, E: ExecutionProivder<N>> {
    pub consensus: C,
    pub execution: Arc<E>,
    filter_state: FilterState,
    block_broadcast: Sender<SubscriptionEvent<N>>,
    fork_schedule: ForkSchedule,
    phantom: PhantomData<N>,
}

impl<N: NetworkSpec, C: Consensus<N::BlockResponse>, E: ExecutionProivder<N>> Node<N, C, E> {
    pub fn new(mut consensus: C, execution: E, fork_schedule: ForkSchedule) -> Self {
        let mut block_recv = consensus.block_recv().unwrap();
        let mut finalized_block_recv = consensus.finalized_block_recv().unwrap();
        let execution = Arc::new(execution);
        let execution_ref = execution.clone();
        let block_broadcast = Sender::new(100);
        let block_broadcast_ref = block_broadcast.clone();

        tokio::spawn(async move {
            loop {
                select! {
                    block = block_recv.recv() => {
                        if let Some(block) = block {
                            execution_ref.push_block(block.clone(), BlockId::Number(BlockNumberOrTag::Latest)).await;
                            _ = block_broadcast_ref.send(SubscriptionEvent::NewHeads(block));
                        }
                    },
                    _ = finalized_block_recv.changed() => {
                        let block = finalized_block_recv.borrow_and_update().clone();
                        if let Some(block) = block {
                            execution_ref.push_block(block, BlockId::Number(BlockNumberOrTag::Finalized)).await;
                        }
                    },
                }
            }
        });

        Node {
            consensus,
            execution,
            filter_state: FilterState::default(),
            block_broadcast,
            fork_schedule,
            phantom: PhantomData::default(),
        }
    }

    async fn check_blocktag_age(&self, block: &BlockTag) -> Result<(), ClientError> {
        match block {
            BlockTag::Latest => self.check_head_age().await,
            BlockTag::Finalized => Ok(()),
            BlockTag::Number(_) => Ok(()),
        }
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
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec, C: Consensus<N::BlockResponse>, E: ExecutionProivder<N>> HeliosApi<N>
    for Node<N, C, E>
{
    async fn shutdown(&self) {
        info!(target: "helios::client","shutting down");
        if let Err(err) = self.consensus.shutdown() {
            warn!(target: "helios::client", error = %err, "graceful shutdown failed");
        }
    }

    async fn wait_synced(&self) {
        let mut interval = interval(Duration::from_millis(100));
        loop {
            interval.tick().await;
            if let Ok(SyncStatus::None) = self.syncing().await {
                break;
            }
        }
    }

    async fn call(&self, tx: &N::TransactionRequest, block: BlockTag) -> Result<Bytes> {
        self.check_blocktag_age(&block).await?;
        let mut evm = Evm::new(
            self.execution.clone(),
            self.chain_id().await,
            self.fork_schedule,
            block,
        );

        Ok(evm.call(tx).await?)
    }

    async fn estimate_gas(&self, tx: &N::TransactionRequest, block: BlockTag) -> Result<u64> {
        self.check_blocktag_age(&block).await?;
        let mut evm = Evm::new(
            self.execution.clone(),
            self.chain_id().await,
            self.fork_schedule,
            block,
        );

        Ok(evm.estimate_gas(tx).await?)
    }

    async fn create_access_list(
        &self,
        tx: &N::TransactionRequest,
        block: BlockTag,
    ) -> Result<AccessListResult> {
        self.check_blocktag_age(&block).await?;
        let mut evm = Evm::new(
            self.execution.clone(),
            self.chain_id().await,
            self.fork_schedule,
            block,
        );

        let res = evm.create_access_list(tx, true).await?;

        Ok(res.access_list_result)
    }

    async fn get_balance(&self, address: Address, tag: BlockTag) -> Result<U256> {
        self.check_blocktag_age(&tag).await?;
        let account = self
            .execution
            .get_account(address, &[], false, tag.into())
            .await?;

        Ok(account.account.balance)
    }

    async fn get_nonce(&self, address: Address, tag: BlockTag) -> Result<u64> {
        self.check_blocktag_age(&tag).await?;
        let account = self
            .execution
            .get_account(address, &[], false, tag.into())
            .await?;

        Ok(account.account.nonce)
    }

    async fn get_block_transaction_count_by_hash(&self, hash: B256) -> Result<Option<u64>> {
        let block = self.execution.get_block(hash.into(), false).await?;
        Ok(block.map(|block| block.transactions().hashes().len() as u64))
    }

    async fn get_block_transaction_count_by_number(&self, tag: BlockTag) -> Result<Option<u64>> {
        let block = self.execution.get_block(tag.into(), false).await?;
        Ok(block.map(|block| block.transactions().hashes().len() as u64))
    }

    async fn get_code(&self, address: Address, tag: BlockTag) -> Result<Bytes> {
        self.check_blocktag_age(&tag).await?;
        let account = self
            .execution
            .get_account(address, &[], true, tag.into())
            .await?;

        account
            .code
            .ok_or(eyre!("Failed to fetch code for address"))
    }

    async fn get_storage_at(&self, address: Address, slot: U256, tag: BlockTag) -> Result<B256> {
        self.check_blocktag_age(&tag).await?;
        self.execution
            .get_account(address, &[slot.into()], false, tag.into())
            .await?
            .get_storage_value(slot.into())
            .ok_or(eyre!("slot not found"))
            .map(|v| v.into())
    }

    async fn get_proof(
        &self,
        address: Address,
        slots: &[B256],
        block: BlockTag,
    ) -> Result<EIP1186AccountProofResponse> {
        self.check_blocktag_age(&block).await?;
        let account = self
            .execution
            .get_account(address, slots, false, block.into())
            .await?;

        Ok(EIP1186AccountProofResponse {
            address,
            balance: account.account.balance,
            code_hash: account.account.code_hash,
            nonce: account.account.nonce,
            storage_hash: account.account.storage_root,
            account_proof: account.account_proof,
            storage_proof: account.storage_proof,
        })
    }

    async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<B256> {
        self.execution.send_raw_transaction(bytes).await
    }

    async fn get_transaction_receipt(&self, tx_hash: B256) -> Result<Option<N::ReceiptResponse>> {
        self.execution.get_receipt(tx_hash).await
    }

    async fn get_block_receipts(&self, block: BlockTag) -> Result<Vec<N::ReceiptResponse>> {
        self.check_blocktag_age(&block).await?;
        self.execution.get_block_receipts(block.into()).await
    }

    async fn get_transaction_by_hash(
        &self,
        tx_hash: B256,
    ) -> Result<Option<N::TransactionResponse>> {
        self.execution.get_transaction(tx_hash).await
    }

    async fn get_transaction_by_block_hash_and_index(
        &self,
        hash: B256,
        index: u64,
    ) -> Result<Option<N::TransactionResponse>> {
        self.execution
            .get_transaction_by_location(hash.into(), index)
            .await
    }

    async fn get_transaction_by_block_number_and_index(
        &self,
        block: BlockTag,
        index: u64,
    ) -> Result<Option<N::TransactionResponse>> {
        self.check_blocktag_age(&block).await?;
        self.execution
            .get_transaction_by_location(block.into(), index)
            .await
    }

    async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>> {
        self.execution.get_logs(filter).await
    }

    async fn client_version(&self) -> String {
        let helios_version = std::env!("CARGO_PKG_VERSION");
        format!("helios-{}", helios_version)
    }

    // async fn get_filter_changes(&self, filter_id: U256) -> Result<FilterChanges> {
    //     todo!()
    //     // match self.filter_state.get_filter(filter_id).await {
    //     //     Some(FilterType::Logs { filter, last_poll}) => {
    //     //         match filter.block_option {
    //     //             FilterBlockOption::Range { from_block, to_block } => {
    //     //
    //     //             },
    //     //             FilterBlockOption::AtBlockHash(hash) => {

    //     //             },
    //     //         }
    //     //     },
    //     //     Some(FilterType::Blocks { .. }) => todo!("implement block filter"),
    //     //     None => Err(eyre!("filter not found")),
    //     // }
    // }

    async fn get_filter_logs(&self, filter_id: U256) -> Result<Vec<Log>> {
        match self.filter_state.get_filter(filter_id).await {
            Some(FilterType::Logs { filter, .. }) => self.get_logs(&filter).await,
            Some(FilterType::Blocks { .. }) => Err(eyre!("expected log filter")),
            None => Err(eyre!("filter not found")),
        }
    }

    async fn uninstall_filter(&self, filter_id: U256) -> Result<bool> {
        Ok(self.filter_state.uninstall_filter(filter_id).await)
    }

    async fn new_filter(&self, filter: &Filter) -> Result<U256> {
        Ok(self.filter_state.new_filter(filter.clone()).await)
    }

    async fn new_block_filter(&self) -> Result<U256> {
        let current_block = self.get_block_number().await?.try_into()?;
        Ok(self.filter_state.new_block_filter(current_block).await)
    }

    async fn new_pending_transaction_filter(&self) -> Result<U256> {
        Err(eyre!("pending transaction filters not supported"))
    }

    async fn get_gas_price(&self) -> Result<U256> {
        self.check_head_age().await?;
        let tag = BlockTag::Latest;
        let block = self
            .execution
            .get_block(tag.into(), false)
            .await?
            .ok_or(eyre!(ClientError::BlockNotFound(tag)))?;

        let base_fee = block.header().base_fee_per_gas().unwrap_or(0_u64);
        // assumes 1 gwei tip
        let tip = 10_u64.pow(9);

        Ok(U256::from(base_fee + tip))
    }

    async fn blob_base_fee(&self, block: BlockTag) -> Result<U256> {
        let block = self
            .execution
            .get_block(block.into(), false)
            .await?
            .ok_or(eyre!(ClientError::BlockNotFound(block)))?;

        if let Some(excess_blob_gas) = block.header().excess_blob_gas() {
            let is_prague = block.header().timestamp() >= self.fork_schedule.prague_timestamp;
            let price = BlobExcessGasAndPrice::new(excess_blob_gas, is_prague).blob_gasprice;
            Ok(U256::from(price))
        } else {
            Ok(U256::ZERO)
        }
    }

    async fn get_priority_fee(&self) -> Result<U256> {
        // assumes 1 gwei tip
        let tip = U256::from(10_u64.pow(9));
        Ok(tip)
    }

    async fn get_block_number(&self) -> Result<U256> {
        self.check_head_age().await?;
        let tag = BlockTag::Latest;
        let block = self
            .execution
            .get_block(tag.into(), false)
            .await?
            .ok_or(eyre!(ClientError::BlockNotFound(tag)))?;

        Ok(U256::from(block.header().number()))
    }

    async fn get_block_by_number(
        &self,
        tag: BlockTag,
        full_tx: bool,
    ) -> Result<Option<N::BlockResponse>> {
        self.check_blocktag_age(&tag).await?;
        self.execution.get_block(tag.into(), full_tx).await
    }

    async fn get_block_by_hash(
        &self,
        hash: B256,
        full_tx: bool,
    ) -> Result<Option<N::BlockResponse>> {
        self.execution.get_block(hash.into(), full_tx).await
    }

    async fn chain_id(&self) -> u64 {
        self.consensus.chain_id()
    }

    async fn syncing(&self) -> Result<SyncStatus> {
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

    async fn get_coinbase(&self) -> Result<Address> {
        Ok(Address::ZERO)
    }

    async fn subscribe(&self, sub_type: SubscriptionType) -> Result<SubEventRx<N>> {
        match sub_type {
            SubscriptionType::NewHeads => Ok(self.block_broadcast.subscribe()),
            _ => Err(eyre::eyre!("Unsupported subscription type: {:?}", sub_type)),
        }
    }
}
