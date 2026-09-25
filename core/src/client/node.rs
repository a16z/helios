use std::marker::PhantomData;
use std::sync::Arc;

use alloy::consensus::BlockHeader;
use alloy::eips::{BlockId, BlockNumberOrTag};
use alloy::network::{primitives::HeaderResponse, BlockResponse, TransactionBuilder};
use alloy::primitives::{Address, Bytes, B256, U256};
use alloy::rpc::types::{
    state::StateOverride, AccessListItem, AccessListResult, EIP1186AccountProofResponse,
    EIP1186StorageProof, Filter, Log, SyncInfo, SyncStatus,
};
use async_trait::async_trait;
use eyre::{eyre, Result};
use revm::context::result::ExecutionResult;
use revm::context_interface::block::BlobExcessGasAndPrice;
use tokio::{select, sync::broadcast::Sender};
use tracing::{info, warn};

use helios_common::{
    execution_provider::ExecutionProvider,
    fork_schedule::ForkSchedule,
    network_spec::NetworkSpec,
    types::{EvmError, SubEventRx, SubscriptionEvent, SubscriptionType},
};

use crate::consensus::Consensus;
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
    phantom: PhantomData<N>,
}

impl<N: NetworkSpec, C: Consensus<N::BlockResponse>, E: ExecutionProvider<N>> Node<N, C, E> {
    pub fn new(mut consensus: C, execution: E, fork_schedule: ForkSchedule) -> Self {
        let mut block_recv = consensus.block_recv().unwrap();
        let mut finalized_block_recv = consensus.finalized_block_recv().unwrap();
        let execution = Arc::new(execution);
        let execution_ref = execution.clone();
        let block_broadcast = Sender::new(100);
        let block_broadcast_ref = block_broadcast.clone();

        #[cfg(not(target_arch = "wasm32"))]
        let run = tokio::spawn;
        #[cfg(target_arch = "wasm32")]
        let run = wasm_bindgen_futures::spawn_local;

        run(async move {
            let mut last_finalized_block_number = None;

            loop {
                select! {
                    block = block_recv.recv() => {
                        match block {
                            Some(block) => {
                                let block_number = block.header().number();
                                let timestamp = block.header().timestamp();

                                // Calculate age of the block
                                let current_time = SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs();

                                let age = current_time.saturating_sub(timestamp);

                                info!(
                                    target: "helios::client",
                                    "latest block     number={} age={}s",
                                    block_number,
                                    age
                                );

                                execution_ref.push_block(
                                    block.clone(),
                                    BlockId::Number(BlockNumberOrTag::Latest)
                                ).await;

                                _ = block_broadcast_ref.send(SubscriptionEvent::NewHeads(block));
                            }
                            None => {
                                // Sender dropped, consensus task has exited - client is no longer usable
                                warn!(target: "helios::client", "consensus client stopped, shut Helios down manually");
                                break;
                            }
                        }
                    },
                    finalized_changed = finalized_block_recv.changed() => {
                        if finalized_changed.is_err() {
                            warn!(target: "helios::client", "consensus client stopped, shut Helios down manually");
                            break;
                        }
                        let block = finalized_block_recv.borrow_and_update().clone();
                        if let Some(block) = block {
                            let block_number = block.header().number();

                            // Only log if this is a new finalized block
                            if last_finalized_block_number != Some(block_number) {
                                info!(
                                    target: "helios::client",
                                    "finalized block  number={}",
                                    block_number
                                );
                                last_finalized_block_number = Some(block_number);
                            }

                            execution_ref.push_block(
                                block,
                                BlockId::Number(BlockNumberOrTag::Finalized)
                            ).await;
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
        self.consensus.wait_synced().await
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
        let block = self
            .execution
            .get_block(block_id, false)
            .await?
            .ok_or(eyre!("block not found"))?;
        let block_id = block.header().hash().into();
        let ceiling = tx.gas_limit().unwrap_or(block.header().gas_limit());
        let chain_id = self.get_chain_id().await;

        let mut candidate = tx.clone();
        candidate.set_gas_limit(ceiling);
        let (result, ..) = N::transact(
            &candidate,
            false,
            self.execution.clone(),
            chain_id,
            self.fork_schedule,
            block_id,
            state_overrides.clone(),
        )
        .await?;

        match &result {
            ExecutionResult::Revert { output, .. } => {
                return Err(EvmError::Revert(Some(output.clone())).into())
            }
            ExecutionResult::Halt { .. } => return Err(EvmError::Revert(None).into()),
            ExecutionResult::Success { .. } => (),
        }

        // Billed gas excludes refunds and does not account for gas retained by
        // EIP-150 calls. Only return a limit that actually succeeds at this state.
        let mut gas = match result {
            ExecutionResult::Success {
                gas_used,
                gas_refunded,
                ..
            } => gas_used.saturating_add(gas_refunded),
            _ => unreachable!("execution success checked above"),
        };
        loop {
            if gas > ceiling {
                return Err(eyre!("gas required exceeds allowance"));
            }
            candidate.set_gas_limit(gas);
            let (result, ..) = N::transact(
                &candidate,
                false,
                self.execution.clone(),
                chain_id,
                self.fork_schedule,
                block_id,
                state_overrides.clone(),
            )
            .await?;
            if result.is_success() {
                return Ok(gas);
            }
            if gas == ceiling {
                return Err(eyre!("gas required exceeds allowance"));
            }
            gas = gas.saturating_add((gas / 10).max(1_000)).min(ceiling);
        }
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
            gas_used: U256::from(result.gas_used()),
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
    use super::*;
    use alloy::{
        primitives::{address, keccak256},
        rpc::types::{Block, Transaction, TransactionReceipt, TransactionRequest},
    };
    use helios_common::{execution_provider::*, types::Account};
    use helios_ethereum::spec::Ethereum;
    use tokio::sync::{mpsc, watch};

    struct TestConsensus;
    #[async_trait]
    impl Consensus<Block<Transaction>> for TestConsensus {
        fn block_recv(&mut self) -> Option<mpsc::Receiver<Block<Transaction>>> {
            None
        }
        fn finalized_block_recv(&mut self) -> Option<watch::Receiver<Option<Block<Transaction>>>> {
            None
        }
        fn checkpoint_recv(&self) -> Option<watch::Receiver<Option<B256>>> {
            None
        }
        fn expected_highest_block(&self) -> u64 {
            1
        }
        fn chain_id(&self) -> u64 {
            1
        }
        fn shutdown(&self) -> Result<()> {
            Ok(())
        }
        async fn wait_synced(&self) -> Result<()> {
            Ok(())
        }
    }
    struct State {
        block: Block<Transaction>,
        code: Bytes,
    }
    const RECIPIENT: Address = address!("2222222222222222222222222222222222222222");
    #[async_trait]
    impl AccountProvider<Ethereum> for State {
        async fn get_account(
            &self,
            address: Address,
            slots: &[B256],
            _: bool,
            block: BlockId,
        ) -> Result<Account> {
            assert_eq!(block, self.block.header.hash.into());
            let code = if address == RECIPIENT {
                self.code.clone()
            } else {
                Bytes::new()
            };
            Ok(Account {
                account: alloy::consensus::TrieAccount {
                    balance: U256::from(1_000_000_000u64),
                    code_hash: keccak256(&code),
                    ..Default::default()
                },
                code: Some(code),
                storage_proof: slots
                    .iter()
                    .map(|slot| EIP1186StorageProof {
                        key: (*slot).into(),
                        value: U256::from(1),
                        proof: vec![],
                    })
                    .collect(),
                ..Default::default()
            })
        }
    }
    #[async_trait]
    impl BlockProvider<Ethereum> for State {
        async fn get_block(&self, _: BlockId, _: bool) -> Result<Option<Block<Transaction>>> {
            Ok(Some(self.block.clone()))
        }
        async fn get_untrusted_block(
            &self,
            _: BlockId,
            _: bool,
        ) -> Result<Option<Block<Transaction>>> {
            unreachable!()
        }
        async fn push_block(&self, _: Block<Transaction>, _: BlockId) {
            unreachable!()
        }
    }
    #[async_trait]
    impl ExecutionHintProvider<Ethereum> for State {
        async fn get_execution_hint(
            &self,
            _: &TransactionRequest,
            _: bool,
            _: BlockId,
        ) -> Result<std::collections::HashMap<Address, Account>> {
            Ok(Default::default())
        }
    }
    #[async_trait]
    impl TransactionProvider<Ethereum> for State {
        async fn get_transaction(&self, _: B256) -> Result<Option<Transaction>> {
            unreachable!()
        }
        async fn get_transaction_by_location(
            &self,
            _: BlockId,
            _: u64,
        ) -> Result<Option<Transaction>> {
            unreachable!()
        }
        async fn send_raw_transaction(&self, _: &[u8]) -> Result<B256> {
            unreachable!()
        }
    }
    #[async_trait]
    impl ReceiptProvider<Ethereum> for State {
        async fn get_receipt(&self, _: B256) -> Result<Option<TransactionReceipt>> {
            unreachable!()
        }
        async fn get_block_receipts(&self, _: BlockId) -> Result<Option<Vec<TransactionReceipt>>> {
            unreachable!()
        }
    }
    #[async_trait]
    impl LogProvider<Ethereum> for State {
        async fn get_logs(&self, _: &Filter) -> Result<Vec<Log>> {
            unreachable!()
        }
    }
    impl ExecutionProvider<Ethereum> for State {}

    fn node(code: Bytes) -> Node<Ethereum, TestConsensus, State> {
        let forks = helios_ethereum::config::networks::mainnet().execution_forks;
        let mut block: Block<Transaction> = Block::default();
        block.header.number = 1;
        block.header.hash = B256::repeat_byte(1);
        block.header.timestamp = forks.prague_timestamp;
        block.header.gas_limit = 1_000_000;
        Node {
            consensus: TestConsensus,
            execution: Arc::new(State { block, code }),
            filter_state: FilterState::default(),
            block_broadcast: Sender::new(1),
            fork_schedule: forks,
            phantom: PhantomData,
        }
    }
    fn request() -> TransactionRequest {
        TransactionRequest {
            to: Some(RECIPIENT.into()),
            gas: Some(1_000_000),
            ..Default::default()
        }
    }
    #[tokio::test]
    async fn rejects_reverting_and_halting_estimates() {
        for code in [
            alloy::primitives::bytes!("60006000fd"),
            alloy::primitives::bytes!("fe"),
        ] {
            let node = node(code);
            let result = node
                .estimate_gas(&request(), Some(B256::repeat_byte(1).into()), None)
                .await;
            assert!(
                result.is_err(),
                "returned a successful estimate for failed execution"
            );
        }
    }
    #[tokio::test]
    async fn estimate_covers_gas_spent_before_refunds() {
        let node = node(alloy::primitives::bytes!("600060005500"));
        let block_id = B256::repeat_byte(1).into();
        let mut tx = request();
        let (initial, _) = Ethereum::transact(
            &tx,
            false,
            node.execution.clone(),
            1,
            node.fork_schedule,
            block_id,
            None,
        )
        .await
        .unwrap();
        assert!(initial.is_success());
        let gas = node.estimate_gas(&tx, Some(block_id), None).await.unwrap();
        assert!(
            gas > initial.gas_used(),
            "refunded gas is not a safe execution limit"
        );
        tx.gas = Some(gas);
        let (result, _) = Ethereum::transact(
            &tx,
            false,
            node.execution.clone(),
            1,
            node.fork_schedule,
            block_id,
            None,
        )
        .await
        .unwrap();
        assert!(result.is_success());
    }
}
