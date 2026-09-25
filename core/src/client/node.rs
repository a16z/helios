use std::marker::PhantomData;
use std::sync::Arc;

use alloy::consensus::BlockHeader;
use alloy::eips::{BlockId, BlockNumberOrTag};
use alloy::network::{primitives::HeaderResponse, BlockResponse};
use alloy::primitives::{Address, Bytes, B256, U256};
use alloy::rpc::types::{
    state::StateOverride, AccessListItem, AccessListResult, EIP1186AccountProofResponse,
    EIP1186StorageProof, Filter, Log, SyncInfo, SyncStatus,
};
use async_trait::async_trait;
use eyre::{eyre, Result};
use revm::context::result::ExecutionResult;
use revm::context_interface::block::BlobExcessGasAndPrice;
use tokio::{
    select,
    sync::{broadcast::Sender, watch},
};
use tracing::{info, warn};

use helios_common::{
    execution_provider::ExecutionProvider,
    fork_schedule::ForkSchedule,
    network_spec::NetworkSpec,
    types::{EvmError, SubEventRx, SubscriptionEvent, SubscriptionType},
};

use crate::consensus::{Consensus, TrustedBlockRef};
use crate::errors::ClientError;
use crate::execution::filter_state::{FilterState, FilterType};
use crate::time::{SystemTime, UNIX_EPOCH};

use super::api::HeliosApi;

pub struct Node<N: NetworkSpec, C: Consensus<N::BlockResponse>, E: ExecutionProvider<N>> {
    pub consensus: C,
    pub execution: Arc<E>,
    filter_state: FilterState,
    block_broadcast: Sender<SubscriptionEvent<N>>,
    fork_schedule: ForkSchedule,
    latest_block_status: watch::Receiver<BlockSyncStatus>,
    finalized_block_status: watch::Receiver<BlockSyncStatus>,
    finalized_block_watch: watch::Receiver<Option<TrustedBlockRef<N::BlockResponse>>>,
    phantom: PhantomData<N>,
}

#[derive(Clone)]
enum BlockSyncStatus {
    Pending,
    Ready,
    Failed(String),
}

async fn wait_for_initial_block(mut status: watch::Receiver<BlockSyncStatus>) -> Result<()> {
    loop {
        let current = status.borrow_and_update().clone();
        match current {
            BlockSyncStatus::Ready => return Ok(()),
            BlockSyncStatus::Failed(error) => return Err(eyre!(error)),
            BlockSyncStatus::Pending => status.changed().await.map_err(|_| {
                eyre!("consensus stopped before the initial execution block was cached")
            })?,
        }
    }
}

impl<N: NetworkSpec, C: Consensus<N::BlockResponse>, E: ExecutionProvider<N>> Node<N, C, E> {
    pub fn new(mut consensus: C, execution: E, fork_schedule: ForkSchedule) -> Self {
        let mut block_recv = consensus.block_recv().unwrap();
        let mut finalized_block_recv = consensus.finalized_block_recv().unwrap();
        let finalized_block_watch = finalized_block_recv.clone();
        let (latest_block_send, latest_block_status) = watch::channel(BlockSyncStatus::Pending);
        let (finalized_block_send, finalized_block_status) =
            watch::channel(BlockSyncStatus::Pending);
        let execution = Arc::new(execution);
        let execution_ref = execution.clone();
        let block_broadcast = Sender::new(100);
        let block_broadcast_ref = block_broadcast.clone();

        #[cfg(not(target_arch = "wasm32"))]
        let run = tokio::spawn;
        #[cfg(target_arch = "wasm32")]
        let run = wasm_bindgen_futures::spawn_local;

        run(async move {
            // BlockCache updates can clear history, so keep writes serialized
            // while allowing the network fetches themselves to overlap.
            let cache_update = tokio::sync::Mutex::new(());
            // Resolve the two heads independently so a slow finalized fetch cannot
            // delay the latest head (or add a sequential RPC round trip).
            let latest = async {
                while let Some(mut trusted_block) = block_recv.recv().await {
                    while let Ok(newer) = block_recv.try_recv() {
                        trusted_block = newer;
                    }
                    let block = match resolve_trusted_block::<N, E>(
                        execution_ref.as_ref(),
                        trusted_block,
                    )
                    .await
                    {
                        Ok(block) => block,
                        Err(err) => {
                            warn!(target: "helios::client", error = %err, "failed to resolve trusted latest block");
                            if !matches!(*latest_block_send.borrow(), BlockSyncStatus::Ready) {
                                latest_block_send.send_replace(BlockSyncStatus::Failed(format!(
                                    "failed to resolve initial latest block: {err}"
                                )));
                            }
                            continue;
                        }
                    };
                    let block_number = block.header().number();
                    let timestamp = block.header().timestamp();
                    let current_time = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let age = current_time.saturating_sub(timestamp);
                    info!(target: "helios::client", "latest block     number={} age={}s", block_number, age);

                    let _cache_update = cache_update.lock().await;
                    execution_ref
                        .push_block(block.clone(), BlockId::Number(BlockNumberOrTag::Latest))
                        .await;
                    latest_block_send.send_replace(BlockSyncStatus::Ready);
                    _ = block_broadcast_ref.send(SubscriptionEvent::NewHeads(block));
                }
            };
            let finalized = async {
                let mut last_finalized_block_number = None;
                while finalized_block_recv.changed().await.is_ok() {
                    let trusted_block = finalized_block_recv.borrow_and_update().clone();
                    let Some(trusted_block) = trusted_block else {
                        continue;
                    };
                    let block = match resolve_trusted_block::<N, E>(
                        execution_ref.as_ref(),
                        trusted_block,
                    )
                    .await
                    {
                        Ok(block) => block,
                        Err(err) => {
                            warn!(target: "helios::client", error = %err, "failed to resolve trusted finalized block");
                            if !matches!(*finalized_block_send.borrow(), BlockSyncStatus::Ready) {
                                finalized_block_send.send_replace(BlockSyncStatus::Failed(
                                    format!("failed to resolve initial finalized block: {err}"),
                                ));
                            }
                            continue;
                        }
                    };
                    let block_number = block.header().number();
                    if last_finalized_block_number != Some(block_number) {
                        info!(target: "helios::client", "finalized block  number={}", block_number);
                        last_finalized_block_number = Some(block_number);
                    }
                    let _cache_update = cache_update.lock().await;
                    execution_ref
                        .push_block(block, BlockId::Number(BlockNumberOrTag::Finalized))
                        .await;
                    finalized_block_send.send_replace(BlockSyncStatus::Ready);
                }
            };
            select! {
                _ = latest => {},
                _ = finalized => {},
            }
            warn!(target: "helios::client", "consensus client stopped, shut Helios down manually");
        });

        Node {
            consensus,
            execution,
            filter_state: FilterState::default(),
            block_broadcast,
            fork_schedule,
            latest_block_status,
            finalized_block_status,
            finalized_block_watch,
            phantom: PhantomData,
        }
    }

    async fn check_blocktag_age(&self, block: &BlockId) -> Result<(), ClientError> {
        match block {
            BlockId::Number(number) => match number {
                BlockNumberOrTag::Latest => self.check_head_age().await,
                _ => Ok(()),
            },
            BlockId::Hash(_) => Ok(()),
        }
    }

    async fn check_head_age(&self) -> Result<(), ClientError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let tag = BlockNumberOrTag::Latest.into();
        let block_timestamp = self
            .execution
            .get_block(tag, false)
            .await
            .map_err(|_| ClientError::BlockNotFound(tag))?
            .ok_or_else(|| ClientError::OutOfSync(timestamp))?
            .header()
            .timestamp();

        let delay = timestamp.saturating_sub(block_timestamp);
        if delay > 60 {
            return Err(ClientError::OutOfSync(delay));
        }

        Ok(())
    }
}

async fn resolve_trusted_block<N: NetworkSpec, E: ExecutionProvider<N>>(
    execution: &E,
    trusted_block: TrustedBlockRef<N::BlockResponse>,
) -> Result<N::BlockResponse> {
    match trusted_block {
        TrustedBlockRef::Full(block) => Ok(block),
        TrustedBlockRef::Hash(block_hash) => {
            let mut block = execution
                .get_untrusted_block(BlockId::Hash(block_hash.into()), true)
                .await?
                .ok_or_else(|| eyre!("trusted execution block {block_hash} not found"))?;

            ensure_trusted_block_valid::<N>(&mut block, block_hash)?;
            Ok(block)
        }
    }
}

fn ensure_trusted_block_valid<N: NetworkSpec>(
    block: &mut N::BlockResponse,
    expected_hash: B256,
) -> Result<()> {
    let block_hash = block.header().hash();

    if block_hash != expected_hash {
        return Err(eyre!(
            "trusted execution block hash mismatch: found {block_hash}, expected {expected_hash}"
        ));
    }

    if !N::validate_block(block, true) {
        return Err(eyre!(
            "trusted execution block {block_hash} failed local hash validation"
        ));
    }

    Ok(())
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec, C: Consensus<N::BlockResponse>, E: ExecutionProvider<N>> HeliosApi<N>
    for Node<N, C, E>
{
    async fn shutdown(&self) {
        info!(target: "helios::client","shutting down");
        if let Err(err) = self.consensus.shutdown() {
            warn!(target: "helios::client", error = %err, "graceful shutdown failed");
        }
    }

    async fn wait_synced(&self) -> Result<()> {
        self.consensus.wait_synced().await?;
        // Ethereum publishes both heads before consensus reports Synced. Other
        // networks (e.g. OP Stack) may only publish a latest head.
        let has_finalized_head = self.finalized_block_watch.borrow().is_some();
        tokio::try_join!(
            wait_for_initial_block(self.latest_block_status.clone()),
            async {
                if has_finalized_head {
                    wait_for_initial_block(self.finalized_block_status.clone()).await?;
                }
                Ok::<_, eyre::Report>(())
            },
        )?;
        Ok(())
    }

    async fn call(
        &self,
        tx: &N::TransactionRequest,
        block_id: BlockId,
        state_overrides: Option<StateOverride>,
    ) -> Result<Bytes> {
        self.check_blocktag_age(&block_id).await?;
        let (result, ..) = N::transact(
            tx,
            false,
            self.execution.clone(),
            self.get_chain_id().await,
            self.fork_schedule,
            block_id,
            state_overrides,
        )
        .await?;

        let res = match result {
            ExecutionResult::Success { output, .. } => Ok(output.into_data()),
            ExecutionResult::Revert { output, .. } => {
                Err(EvmError::Revert(Some(output.to_vec().into())))
            }
            ExecutionResult::Halt { .. } => Err(EvmError::Revert(None)),
        }?;

        Ok(res)
    }

    async fn estimate_gas(
        &self,
        tx: &N::TransactionRequest,
        block_id: Option<BlockId>,
        state_overrides: Option<StateOverride>,
    ) -> Result<u64> {
        let block_id = block_id.unwrap_or(BlockId::latest());
        self.check_blocktag_age(&block_id).await?;

        let (result, ..) = N::transact(
            tx,
            false,
            self.execution.clone(),
            self.get_chain_id().await,
            self.fork_schedule,
            block_id,
            state_overrides,
        )
        .await?;

        Ok(result.tx_gas_used())
    }

    async fn create_access_list(
        &self,
        tx: &N::TransactionRequest,
        block: BlockId,
        state_overrides: Option<StateOverride>,
    ) -> Result<AccessListResult> {
        self.check_blocktag_age(&block).await?;

        let (result, accounts) = N::transact(
            tx,
            false,
            self.execution.clone(),
            self.get_chain_id().await,
            self.fork_schedule,
            block,
            state_overrides,
        )
        .await?;

        let access_list_result = AccessListResult {
            access_list: accounts
                .iter()
                .map(|(address, account)| {
                    let storage_keys = account
                        .storage_proof
                        .iter()
                        .map(|EIP1186StorageProof { key, .. }| key.as_b256())
                        .collect();
                    AccessListItem {
                        address: *address,
                        storage_keys,
                    }
                })
                .collect::<Vec<_>>()
                .into(),
            gas_used: U256::from(result.tx_gas_used()),
            error: matches!(result, ExecutionResult::Revert { .. })
                .then_some(result.output().unwrap().to_string()),
        };

        Ok(access_list_result)
    }

    async fn get_balance(&self, address: Address, block_id: BlockId) -> Result<U256> {
        self.check_blocktag_age(&block_id).await?;
        let account = self
            .execution
            .get_account(address, &[], false, block_id)
            .await?;

        Ok(account.account.balance)
    }

    async fn get_nonce(&self, address: Address, block_id: BlockId) -> Result<u64> {
        self.check_blocktag_age(&block_id).await?;
        let account = self
            .execution
            .get_account(address, &[], false, block_id)
            .await?;

        Ok(account.account.nonce)
    }

    async fn get_block_transaction_count(&self, block_id: BlockId) -> Result<Option<u64>> {
        let block = self.execution.get_block(block_id, false).await?;
        Ok(block.map(|block| block.transactions().hashes().len() as u64))
    }

    async fn get_code(&self, address: Address, block_id: BlockId) -> Result<Bytes> {
        self.check_blocktag_age(&block_id).await?;
        let account = self
            .execution
            .get_account(address, &[], true, block_id)
            .await?;

        account
            .code
            .ok_or(eyre!("Failed to fetch code for address"))
    }

    async fn get_storage_at(
        &self,
        address: Address,
        slot: U256,
        block_id: BlockId,
    ) -> Result<B256> {
        self.check_blocktag_age(&block_id).await?;
        self.execution
            .get_account(address, &[slot.into()], false, block_id)
            .await?
            .get_storage_value(slot.into())
            .ok_or(eyre!("slot not found"))
            .map(|v| v.into())
    }

    async fn get_proof(
        &self,
        address: Address,
        slots: &[B256],
        block_id: BlockId,
    ) -> Result<EIP1186AccountProofResponse> {
        self.check_blocktag_age(&block_id).await?;
        let account = self
            .execution
            .get_account(address, slots, false, block_id)
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

    async fn get_block_receipts(
        &self,
        block_id: BlockId,
    ) -> Result<Option<Vec<N::ReceiptResponse>>> {
        self.check_blocktag_age(&block_id).await?;
        self.execution.get_block_receipts(block_id).await
    }

    async fn get_transaction(&self, tx_hash: B256) -> Result<Option<N::TransactionResponse>> {
        self.execution.get_transaction(tx_hash).await
    }

    async fn get_transaction_by_block_and_index(
        &self,
        block_id: BlockId,
        index: u64,
    ) -> Result<Option<N::TransactionResponse>> {
        self.check_blocktag_age(&block_id).await?;
        self.execution
            .get_transaction_by_location(block_id, index)
            .await
    }

    async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>> {
        self.execution.get_logs(filter).await
    }

    async fn get_client_version(&self) -> String {
        let helios_version = std::env!("CARGO_PKG_VERSION");
        format!("helios-{}", helios_version)
    }

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

    async fn get_gas_price(&self) -> Result<U256> {
        self.check_head_age().await?;
        let block_id = BlockNumberOrTag::Latest.into();
        let block = self
            .execution
            .get_block(block_id, false)
            .await?
            .ok_or(eyre!(ClientError::BlockNotFound(block_id)))?;

        let base_fee = block.header().base_fee_per_gas().unwrap_or(0_u64);
        // assumes 1 gwei tip
        let tip = 10_u64.pow(9);

        Ok(U256::from(base_fee + tip))
    }

    async fn get_blob_base_fee(&self) -> Result<U256> {
        let block_id = BlockNumberOrTag::Latest.into();
        let block = self
            .execution
            .get_block(block_id, false)
            .await?
            .ok_or(eyre!(ClientError::BlockNotFound(block_id)))?;

        if let Some(excess_blob_gas) = block.header().excess_blob_gas() {
            // Get blob base fee update fraction based on fork
            let blob_base_fee_update_fraction = self
                .fork_schedule
                .get_blob_base_fee_update_fraction(block.header().timestamp());

            let price = BlobExcessGasAndPrice::new(excess_blob_gas, blob_base_fee_update_fraction)
                .blob_gasprice;
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
        let block_id = BlockNumberOrTag::Latest.into();
        let block = self
            .execution
            .get_block(block_id, false)
            .await?
            .ok_or(eyre!(ClientError::BlockNotFound(block_id)))?;

        Ok(U256::from(block.header().number()))
    }

    async fn get_block(
        &self,
        block_id: BlockId,
        full_tx: bool,
    ) -> Result<Option<N::BlockResponse>> {
        self.check_blocktag_age(&block_id).await?;
        self.execution.get_block(block_id, full_tx).await
    }

    async fn get_chain_id(&self) -> u64 {
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

    async fn current_checkpoint(&self) -> Result<Option<B256>> {
        self.consensus
            .checkpoint_recv()
            .map(|recv| *recv.borrow())
            .ok_or_else(|| eyre!("Checkpoints not supported"))
    }

    fn new_checkpoints_recv(&self) -> Result<tokio::sync::watch::Receiver<Option<B256>>> {
        self.consensus
            .checkpoint_recv()
            .ok_or_else(|| eyre!("Checkpoints not supported"))
    }
}

#[cfg(test)]
mod tests {
    use super::ensure_trusted_block_valid;
    use alloy::{
        consensus::proofs::{calculate_transaction_root, calculate_withdrawals_root},
        primitives::B256,
        rpc::types::{Block, BlockTransactions},
    };
    use helios_ethereum::spec::Ethereum;

    fn full_block() -> Block {
        let mut block = helios_test_utils::rpc_block();
        let mut tx = helios_test_utils::rpc_tx();
        block.header.transactions_root = calculate_transaction_root(&[tx.inner.clone()]);
        block.header.withdrawals_root = Some(calculate_withdrawals_root(&[]));
        block.withdrawals = Some(Default::default());
        block.header.hash = block.header.hash_slow();
        tx.block_hash = Some(block.header.hash);
        tx.block_number = Some(block.header.number);
        tx.transaction_index = Some(0);
        tx.block_timestamp = Some(u64::MAX);
        block.transactions = BlockTransactions::Full(vec![tx]);
        block
    }

    #[test]
    fn hash_handoff_authenticates_and_normalizes_rpc_block() {
        let mut block = full_block();
        let hash = block.header.hash;
        ensure_trusted_block_valid::<Ethereum>(&mut block, hash).unwrap();
        assert!(block.header.size.is_none());
        assert!(block.header.total_difficulty.is_none());
        assert_eq!(
            block.transactions.as_transactions().unwrap()[0].block_timestamp,
            Some(block.header.timestamp)
        );
    }

    #[test]
    fn hash_handoff_rejects_forged_header_body_and_transaction_metadata() {
        let original = full_block();
        for field in ["hash", "logs_bloom", "requests_hash", "body", "metadata"] {
            let mut block = original.clone();
            match field {
                "hash" => block.header.hash = B256::ZERO,
                "logs_bloom" => block.header.logs_bloom = Default::default(),
                "requests_hash" => block.header.requests_hash = Some(B256::ZERO),
                "body" => block.transactions = BlockTransactions::Full(vec![]),
                "metadata" => {
                    let BlockTransactions::Full(txs) = &mut block.transactions else {
                        unreachable!()
                    };
                    txs[0].block_number = None;
                }
                _ => unreachable!(),
            }
            assert!(
                ensure_trusted_block_valid::<Ethereum>(&mut block, original.header.hash).is_err(),
                "accepted forged {field}"
            );
        }
    }
}
