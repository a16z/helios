// use std::collections::HashMap;
// use std::sync::Arc;
//
// use alloy::consensus::BlockHeader;
// use alloy::eips::BlockId;
// use alloy::network::primitives::HeaderResponse;
// use alloy::network::BlockResponse;
// use alloy::primitives::{Address, B256, U256};
// use alloy::rpc::types::{EIP1186AccountProofResponse, Filter, FilterChanges, Log};
// use async_trait::async_trait;
// use eyre::Result;
// use revm::context_interface::block::BlobExcessGasAndPrice;
// use tracing::warn;
//
// use helios_common::{
//     fork_schedule::ForkSchedule,
//     network_spec::NetworkSpec,
//     types::{Account, BlockTag, SubEventRx, SubscriptionType},
// };
//
// use self::client::ExecutionInner;
// use self::errors::ExecutionError;
// use self::providers::ExecutionProivder;
// use self::spec::ExecutionSpec;
// use self::state::{FilterType, State};
//
pub mod client;
pub mod constants;
pub mod errors;
pub mod evm;
pub mod filter_state;
pub mod proof;
pub mod providers;
pub mod rpc;
pub mod spec;
pub mod state;
//
// // #[derive(Clone)]
// pub struct ExecutionClient<N: NetworkSpec, E: ExecutionProivder<N>> {
//     provider: E,
//     fork_schedule: ForkSchedule,
// }
//
// impl<N: NetworkSpec, E: ExecutionProivder<N>> ExecutionClient<N, E> {
//     pub fn new(provider: E, fork_schedule: ForkSchedule) -> Self {
//         Self {
//             provider,
//             fork_schedule,
//         }
//     }
//
//     async fn get_account(
//         &self,
//         address: Address,
//         slots: &[B256],
//         include_code: bool,
//         block_id: BlockId,
//     ) -> Result<Account> {
//         self.provider
//             .get_account(address, slots, include_code, block_id)
//             .await
//     }
//
//     pub async fn get_storage_at(
//         &self,
//         address: Address,
//         slot: U256,
//         block: BlockTag,
//     ) -> Result<B256> {
//         let storage_key = slot.into();
//
//         let account = self
//             .get_account(address, Some(&[storage_key]), block, false)
//             .await?;
//
//         let value = account.get_storage_value(storage_key);
//         match value {
//             Some(value) => Ok(value.into()),
//             None => Err(ExecutionError::InvalidStorageProof(address, storage_key).into()),
//         }
//     }
//
//     pub async fn get_proof(
//         &self,
//         address: Address,
//         slots: &[B256],
//         block: BlockTag,
//     ) -> Result<EIP1186AccountProofResponse> {
//         let account = self
//             .provider
//             .get_account(address, slots, false, block.into())
//             .await?;
//
//         Ok(EIP1186AccountProofResponse {
//             address,
//             balance: account.account.balance,
//             code_hash: account.account.code_hash,
//             nonce: account.account.nonce,
//             storage_hash: account.account.storage_root,
//             account_proof: account.account_proof,
//             storage_proof: account.storage_proof,
//         })
//     }
//
//     pub async fn blob_base_fee(&self, _block: BlockTag) -> U256 {
//         // TODO: fix
//         U256::ZERO
//
//         // let block = self.provider.get_block(block.into(), false).await;
//         // let Ok(Some(block)) = block else {
//         //     warn!(target: "helios::execution", "requested block not found");
//         //     return U256::from(0);
//         // };
//
//         // let parent_hash = block.header().parent_hash();
//         // let parent_block = self.state.get_block_by_hash(parent_hash).await;
//         // if parent_block.is_none() {
//         //     warn!(target: "helios::execution", "requested parent block not found");
//         //     return U256::from(0);
//         // };
//
//         // let excess_blob_gas = parent_block.unwrap().header().excess_blob_gas().unwrap();
//         // let is_prague = block.header().timestamp() >= self.fork_schedule.prague_timestamp;
//         // U256::from(BlobExcessGasAndPrice::new(excess_blob_gas, is_prague).blob_gasprice)
//     }
//
//     pub async fn get_transaction_by_location(
//         &self,
//         block_tag: BlockTag,
//         index: u64,
//     ) -> Result<Option<N::TransactionResponse>> {
//         self.provider
//             .get_transaction_by_location(block_tag.into(), index)
//             .await
//     }
//
//     pub async fn get_transaction(&self, hash: B256) -> Option<N::TransactionResponse> {
//         self.provider.get_transaction(hash).await
//     }
//
//     // TODO: fixme
//     // pub async fn subscribe(&self, sub_type: SubscriptionType) -> Result<SubEventRx<N>> {
//     //     match sub_type {
//     //         SubscriptionType::NewHeads => Ok(self.state.subscribe_blocks().await),
//     //         _ => Err(eyre::eyre!("Unsupported subscription type: {:?}", sub_type)),
//     //     }
//     // }
// }
//
// #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
// #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
// impl<N: NetworkSpec> ExecutionSpec<N> for ExecutionClient<N> {
//     async fn get_account(
//         &self,
//         address: Address,
//         slots: Option<&[B256]>,
//         tag: BlockTag,
//         include_code: bool,
//     ) -> Result<Account> {
//         self.client
//             .get_account(address, slots, tag, include_code)
//             .await
//     }
//
//     async fn get_transaction_receipt(&self, tx_hash: B256) -> Result<Option<N::ReceiptResponse>> {
//         self.client.get_transaction_receipt(tx_hash).await
//     }
//
//     async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>> {
//         let filter = filter.clone();
//
//         // avoid fetching logs for a block helios hasn't seen yet
//         let filter = if filter.get_to_block().is_none() && filter.get_block_hash().is_none() {
//             let block = self.state.latest_block_number().await.unwrap();
//             let filter = filter.to_block(block);
//             if filter.get_from_block().is_none() {
//                 filter.from_block(block)
//             } else {
//                 filter
//             }
//         } else {
//             filter
//         };
//
//         let logs = self.client.get_logs(&filter).await?;
//         ensure_logs_match_filter(&logs, &filter)?;
//         Ok(logs)
//     }
//
//     async fn get_filter_changes(&self, filter_id: U256) -> Result<FilterChanges> {
//         let filter_type = self.state.get_filter(&filter_id).await;
//
//         Ok(match &filter_type {
//             None => {
//                 // only concerned with filters created via helios
//                 return Err(ExecutionError::FilterNotFound(filter_id).into());
//             }
//             Some(FilterType::Logs(filter)) => {
//                 // underlying RPC takes care of keeping track of changes
//                 let filter_changes = self.client.get_filter_changes(filter_id).await?;
//                 let logs = filter_changes.as_logs().unwrap_or(&[]);
//                 ensure_logs_match_filter(logs, filter)?;
//                 FilterChanges::Logs(logs.to_vec())
//             }
//             Some(FilterType::NewBlock(last_block_num)) => {
//                 let blocks = self
//                     .state
//                     .get_blocks_after(BlockTag::Number(*last_block_num))
//                     .await;
//                 if !blocks.is_empty() {
//                     // keep track of the last block number in state
//                     // so next call can filter starting from the prev call's (last block number + 1)
//                     self.state
//                         .push_filter(
//                             filter_id,
//                             FilterType::NewBlock(blocks.last().unwrap().header().number()),
//                         )
//                         .await;
//                 }
//                 let block_hashes = blocks.into_iter().map(|b| b.header().hash()).collect();
//                 FilterChanges::Hashes(block_hashes)
//             }
//             Some(FilterType::PendingTransactions) => {
//                 // underlying RPC takes care of keeping track of changes
//                 let filter_changes = self.client.get_filter_changes(filter_id).await?;
//                 let tx_hashes = filter_changes.as_hashes().unwrap_or(&[]);
//                 FilterChanges::Hashes(tx_hashes.to_vec())
//             }
//         })
//     }
//
//     async fn get_filter_logs(&self, filter_id: U256) -> Result<Vec<Log>> {
//         let filter_type = self.state.get_filter(&filter_id).await;
//
//         match &filter_type {
//             Some(FilterType::Logs(filter)) => {
//                 let logs = self.client.get_filter_logs(filter_id).await?;
//                 ensure_logs_match_filter(&logs, filter)?;
//                 Ok(logs)
//             }
//             _ => {
//                 // only concerned with filters created via helios
//                 Err(ExecutionError::FilterNotFound(filter_id).into())
//             }
//         }
//     }
//
//     async fn create_extended_access_list(
//         &self,
//         tx: &N::TransactionRequest,
//         validate_tx: bool,
//         block: Option<BlockId>,
//     ) -> Result<HashMap<Address, Account>> {
//         self.client
//             .create_extended_access_list(tx, validate_tx, block)
//             .await
//     }
//
//     async fn chain_id(&self) -> Result<u64> {
//         // ToDo: verify the response from RPC/API or just return from config?
//         self.client.chain_id().await
//     }
//
//     async fn get_block(
//         &self,
//         block_id: BlockId,
//         full_tx: bool,
//     ) -> Result<Option<N::BlockResponse>> {
//         self.client.get_block(block_id, full_tx).await
//     }
//
//     async fn get_untrusted_block(
//         &self,
//         block_id: BlockId,
//         full_tx: bool,
//     ) -> Result<Option<N::BlockResponse>> {
//         self.client.get_untrusted_block(block_id, full_tx).await
//     }
//
//     async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<B256> {
//         self.client.send_raw_transaction(bytes).await
//     }
//
//     async fn get_block_receipts(
//         &self,
//         block_id: BlockId,
//     ) -> Result<Option<Vec<N::ReceiptResponse>>> {
//         self.client.get_block_receipts(block_id).await
//     }
//
//     async fn new_filter(&self, filter: &Filter) -> Result<U256> {
//         let filter = filter.clone();
//
//         // avoid submitting a filter for logs for a block helios hasn't seen yet
//         let filter = if filter.get_to_block().is_none() && filter.get_block_hash().is_none() {
//             let block = self.state.latest_block_number().await.unwrap();
//             let filter = filter.to_block(block);
//             if filter.get_from_block().is_none() {
//                 filter.from_block(block)
//             } else {
//                 filter
//             }
//         } else {
//             filter
//         };
//         let filter_id = self.client.new_filter(&filter).await?;
//
//         // record the filter in the state
//         self.state
//             .push_filter(filter_id, FilterType::Logs(Box::new(filter)))
//             .await;
//
//         Ok(filter_id)
//     }
//
//     async fn new_block_filter(&self) -> Result<U256> {
//         let filter_id = self.client.new_block_filter().await?;
//
//         // record the filter in the state
//         let latest_block_num = self.state.latest_block_number().await.unwrap_or(1);
//         self.state
//             .push_filter(filter_id, FilterType::NewBlock(latest_block_num))
//             .await;
//
//         Ok(filter_id)
//     }
//
//     async fn new_pending_transaction_filter(&self) -> Result<U256> {
//         let filter_id = self.client.new_pending_transaction_filter().await?;
//
//         // record the filter in the state
//         self.state
//             .push_filter(filter_id, FilterType::PendingTransactions)
//             .await;
//
//         Ok(filter_id)
//     }
//
//     async fn uninstall_filter(&self, filter_id: U256) -> Result<bool> {
//         // remove the filter from the state
//         self.state.remove_filter(&filter_id).await;
//         self.client.uninstall_filter(filter_id).await
//     }
// }
//
