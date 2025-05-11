use std::collections::{HashMap, HashSet};

use alloy::{
    consensus::{Account as TrieAccount, BlockHeader},
    eips::BlockId,
    network::{BlockResponse, ReceiptResponse, TransactionBuilder},
    primitives::{Address, B256, U256},
    rlp,
    rpc::types::{EIP1186AccountProofResponse, Filter, FilterChanges, Log},
};
use async_trait::async_trait;
use eyre::Result;
use futures::future::{join_all, try_join_all};
use helios_common::{
    network_spec::NetworkSpec,
    types::{Account, BlockTag},
};
use revm::primitives::AccessListItem;

use super::{ExecutionInner, ExecutionSpec};
use crate::execution::{
    constants::{MAX_SUPPORTED_BLOCKS_TO_PROVE_FOR_LOGS, PARALLEL_QUERY_BATCH_SIZE},
    errors::ExecutionError,
    proof::{
        ordered_trie_root_noop_encoder, verify_account_proof, verify_block_receipts,
        verify_code_hash_proof, verify_storage_proof,
    },
    rpc::ExecutionRpc,
    state::State,
};

#[derive(Clone)]
pub struct ExecutionInnerRpcClient<N: NetworkSpec, R: ExecutionRpc<N>> {
    rpc: R,
    state: State<N>,
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec, R: ExecutionRpc<N>> ExecutionInner<N> for ExecutionInnerRpcClient<N, R> {
    fn new(url: &str, state: State<N>) -> Result<Self> {
        let rpc = R::new(url)?;
        Ok(Self { rpc, state })
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec, R: ExecutionRpc<N>> ExecutionSpec<N> for ExecutionInnerRpcClient<N, R> {
    async fn get_account(
        &self,
        address: Address,
        slots: Option<&[B256]>,
        tag: BlockTag,
        include_code: bool,
    ) -> Result<Account> {
        let slots = slots.unwrap_or(&[]);
        let block = self
            .state
            .get_block(tag)
            .await
            .ok_or(ExecutionError::BlockNotFound(tag))?;

        let proof = self
            .rpc
            .get_proof(address, slots, block.header().number().into())
            .await?;

        self.verify_account_proof(proof, &block, include_code).await
    }

    async fn get_transaction_receipt(&self, tx_hash: B256) -> Result<Option<N::ReceiptResponse>> {
        let Some(receipt) = self.rpc.get_transaction_receipt(tx_hash).await? else {
            return Ok(None);
        };

        let block_num = receipt.block_number().unwrap();
        let block_id = BlockId::from(block_num);
        let tag = BlockTag::Number(block_num);

        let Some(block_receipts_root) = self.state.get_receipts_root(tag).await else {
            return Ok(None);
        };

        // Fetch all receipts in block, check root and inclusion
        let receipts = self
            .rpc
            .get_block_receipts(block_id)
            .await?
            .ok_or(ExecutionError::NoReceiptsForBlock(tag))?;

        let receipts_encoded = receipts.iter().map(N::encode_receipt).collect::<Vec<_>>();
        let expected_receipt_root = ordered_trie_root_noop_encoder(&receipts_encoded);

        if expected_receipt_root != block_receipts_root
            // Note: Some RPC providers return different response in `eth_getTransactionReceipt` vs `eth_getBlockReceipts`
            // Primarily due to https://github.com/ethereum/execution-apis/issues/295 not finalized
            // Which means that the basic equality check in N::receipt_contains can be flaky
            // So as a fallback do equality check on encoded receipts as well
            || !(
                N::receipt_contains(&receipts, &receipt)
                || receipts_encoded.contains(&N::encode_receipt(&receipt))
            )
        {
            return Err(ExecutionError::ReceiptRootMismatch(tx_hash).into());
        }

        Ok(Some(receipt))
    }

    async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>> {
        let logs = self.rpc.get_logs(filter).await?;

        self.verify_logs(&logs).await?;

        Ok(logs)
    }

    async fn get_filter_changes(&self, filter_id: U256) -> Result<FilterChanges> {
        let filter_changes = self.rpc.get_filter_changes(filter_id).await?;

        if filter_changes.is_logs() {
            let logs = filter_changes.as_logs().unwrap_or(&[]);
            self.verify_logs(logs).await?;
        }

        Ok(filter_changes)
    }

    async fn get_filter_logs(&self, filter_id: U256) -> Result<Vec<Log>> {
        let logs = self.rpc.get_filter_logs(filter_id).await?;

        self.verify_logs(&logs).await?;

        Ok(logs)
    }

    async fn create_extended_access_list(
        &self,
        tx: &N::TransactionRequest,
        _validate_tx: bool,
        block_id: Option<BlockId>,
    ) -> Result<HashMap<Address, Account>> {
        let block_id = block_id.unwrap_or_default();
        let tag = BlockTag::try_from(block_id)?;
        let block = self
            .state
            .get_block(tag)
            .await
            .ok_or(ExecutionError::BlockNotFound(tag))?;
        let block_id = BlockId::Number(block.header().number().into());

        let mut list = self.rpc.create_access_list(tx, block_id).await?.0;

        let from_access_entry = AccessListItem {
            address: tx.from().unwrap_or_default(),
            storage_keys: Vec::default(),
        };
        let to_access_entry = AccessListItem {
            address: tx.to().unwrap_or_default(),
            storage_keys: Vec::default(),
        };
        let producer_access_entry = AccessListItem {
            address: block.header().beneficiary(),
            storage_keys: Vec::default(),
        };

        let list_addresses = list.iter().map(|elem| elem.address).collect::<Vec<_>>();

        if !list_addresses.contains(&from_access_entry.address) {
            list.push(from_access_entry)
        }
        if !list_addresses.contains(&to_access_entry.address) {
            list.push(to_access_entry)
        }
        if !list_addresses.contains(&producer_access_entry.address) {
            list.push(producer_access_entry)
        }

        let mut account_map = HashMap::new();
        for chunk in list.chunks(PARALLEL_QUERY_BATCH_SIZE) {
            let account_chunk_futs = chunk.iter().map(|account| {
                let account_fut = self.get_account(
                    account.address,
                    Some(account.storage_keys.as_slice()),
                    tag,
                    true,
                );
                async move { (account.address, account_fut.await) }
            });

            let account_chunk = join_all(account_chunk_futs).await;

            for (address, value) in account_chunk {
                let account = value?;
                account_map.insert(address, account);
            }
        }

        Ok(account_map)
    }

    async fn chain_id(&self) -> Result<u64> {
        self.rpc.chain_id().await
    }

    async fn get_block(
        &self,
        block_id: BlockId,
        full_tx: bool,
    ) -> Result<Option<N::BlockResponse>> {
        self.state.get_block_by_id(block_id, full_tx).await
    }

    async fn get_untrusted_block(
        &self,
        block_id: BlockId,
        full_tx: bool,
    ) -> Result<Option<N::BlockResponse>> {
        self.rpc.get_block(block_id, full_tx.into()).await
    }

    async fn get_block_receipts(
        &self,
        block_id: BlockId,
    ) -> Result<Option<Vec<N::ReceiptResponse>>> {
        let block = self.state.get_block_by_id(block_id, false).await?;
        let Some(block) = block else {
            return Ok(None);
        };
        let block_num = block.header().number();

        let receipts = self
            .rpc
            .get_block_receipts(block_num.into())
            .await?
            .ok_or(ExecutionError::NoReceiptsForBlock(block_num.into()))?;

        verify_block_receipts::<N>(&receipts, &block)?;

        Ok(Some(receipts))
    }

    async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<B256> {
        self.rpc.send_raw_transaction(bytes).await
    }

    async fn new_filter(&self, filter: &Filter) -> Result<U256> {
        self.rpc.new_filter(filter).await
    }

    async fn new_block_filter(&self) -> Result<U256> {
        self.rpc.new_block_filter().await
    }

    async fn new_pending_transaction_filter(&self) -> Result<U256> {
        self.rpc.new_pending_transaction_filter().await
    }

    async fn uninstall_filter(&self, filter_id: U256) -> Result<bool> {
        self.rpc.uninstall_filter(filter_id).await
    }
}

impl<N: NetworkSpec, R: ExecutionRpc<N>> ExecutionInnerRpcClient<N, R> {
    async fn verify_account_proof(
        &self,
        proof: EIP1186AccountProofResponse,
        block: &N::BlockResponse,
        verify_code: bool,
    ) -> Result<Account> {
        // Verify the account proof
        verify_account_proof(&proof, block.header().state_root())?;
        // Verify the storage proofs
        verify_storage_proof(&proof)?;
        // Verify the code hash
        let code = if verify_code {
            let code = self
                .rpc
                .get_code(proof.address, block.header().number().into())
                .await?
                .into();
            verify_code_hash_proof(&proof, &code)?;
            Some(code)
        } else {
            None
        };

        Ok(Account {
            account: TrieAccount {
                nonce: proof.nonce,
                balance: proof.balance,
                storage_root: proof.storage_hash,
                code_hash: proof.code_hash,
            },
            code,
            account_proof: proof.account_proof,
            storage_proof: proof.storage_proof,
        })
    }

    /// Verify the integrity of each log entry in the given array of logs by
    /// checking its inclusion in the corresponding transaction receipt
    /// and verifying the transaction receipt itself against the block's receipt root.
    async fn verify_logs(&self, logs: &[Log]) -> Result<()> {
        // Collect all (unique) block numbers
        let block_nums = logs
            .iter()
            .map(|log| {
                log.block_number
                    .ok_or_else(|| eyre::eyre!("block number not found in log"))
            })
            .collect::<Result<HashSet<_>, _>>()?;

        // Check if the number of blocks to prove is within the limit
        if block_nums.len() > MAX_SUPPORTED_BLOCKS_TO_PROVE_FOR_LOGS {
            return Err(ExecutionError::TooManyLogsToProve(
                logs.len(),
                block_nums.len(),
                MAX_SUPPORTED_BLOCKS_TO_PROVE_FOR_LOGS,
            )
            .into());
        }

        // Collect all (proven) tx receipts for all block numbers
        let blocks_receipts_fut = block_nums.into_iter().map(|block_num| async move {
            let receipts = self.get_block_receipts(block_num.into()).await;
            receipts?
                .ok_or_else(|| eyre::eyre!(ExecutionError::NoReceiptsForBlock(block_num.into())))
        });
        let blocks_receipts = try_join_all(blocks_receipts_fut).await?;
        let receipts = blocks_receipts.into_iter().flatten().collect::<Vec<_>>();

        // Map tx hashes to encoded logs
        let receipts_logs_encoded = receipts
            .into_iter()
            .filter_map(|receipt| {
                let logs = N::receipt_logs(&receipt);
                if logs.is_empty() {
                    None
                } else {
                    let tx_hash = logs[0].transaction_hash.unwrap();
                    let encoded_logs = logs
                        .iter()
                        .map(|l| rlp::encode(&l.inner))
                        .collect::<Vec<_>>();
                    Some((tx_hash, encoded_logs))
                }
            })
            .collect::<HashMap<_, _>>();

        for log in logs {
            // Check if the receipt contains the desired log
            // Encoding logs for comparison
            let tx_hash = log.transaction_hash.unwrap();
            let log_encoded = rlp::encode(&log.inner);
            let receipt_logs_encoded = receipts_logs_encoded.get(&tx_hash).unwrap();

            if !receipt_logs_encoded.contains(&log_encoded) {
                return Err(ExecutionError::MissingLog(
                    tx_hash,
                    U256::from(log.log_index.unwrap()),
                )
                .into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use alloy::rpc::types::TransactionRequest;
    use helios_ethereum::spec::Ethereum as EthereumSpec;
    use helios_test_utils::*;

    use super::*;
    use crate::execution::rpc::mock_rpc::MockRpc;

    async fn get_client() -> ExecutionInnerRpcClient<EthereumSpec, MockRpc> {
        let base_path = testdata_dir().join("rpc/");
        let state = State::<EthereumSpec>::new(1);
        let client = ExecutionInnerRpcClient::<EthereumSpec, MockRpc>::new(
            base_path.to_str().unwrap(),
            state.clone(),
        )
        .unwrap();
        let block = rpc_block();
        state.push_block(block, Arc::new(client.clone())).await;
        client
    }

    #[tokio::test]
    async fn test_get_account() {
        let client = get_client().await;
        let rpc_proof = rpc_proof();
        let block = rpc_block();

        let response = client
            .get_account(
                rpc_proof.address,
                Some(&[rpc_proof.storage_proof[0].key.as_b256()]),
                BlockTag::Number(block.header().number()),
                true,
            )
            .await
            .unwrap();

        assert_eq!(response, rpc_account());
    }

    #[tokio::test]
    async fn test_get_account_without_code() {
        let client = get_client().await;
        let rpc_proof = rpc_proof();
        let block = rpc_block();

        let response = client
            .get_account(
                rpc_proof.address,
                Some(&[rpc_proof.storage_proof[0].key.as_b256()]),
                BlockTag::Number(block.header().number()),
                false,
            )
            .await
            .unwrap();

        let mut expected_account = rpc_account();
        expected_account.code = None;
        assert_eq!(response, expected_account);
    }

    #[tokio::test]
    async fn test_get_account_block_not_in_state() {
        let client = get_client().await;
        let rpc_proof = rpc_proof();

        let response = client
            .get_account(rpc_proof.address, None, BlockTag::Finalized, true)
            .await;

        assert_eq!(
            response.unwrap_err().to_string(),
            ExecutionError::BlockNotFound(BlockTag::Finalized).to_string()
        );
    }

    #[tokio::test]
    async fn test_get_transaction_receipt() {
        let client = get_client().await;
        let rpc_tx_receipt = rpc_tx_receipt();

        let response = client
            .get_transaction_receipt(rpc_tx_receipt.transaction_hash)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(response, rpc_tx_receipt);
    }

    #[tokio::test]
    async fn test_get_logs() {
        let client = get_client().await;
        let filter = Filter::default();

        let response = client.get_logs(&filter).await.unwrap();

        assert_eq!(response, rpc_logs());
    }

    #[tokio::test]
    async fn test_get_filter_changes_logs() {
        let client = get_client().await;
        let filter_id = rpc_filter_id_logs();

        let response = client.get_filter_changes(filter_id).await.unwrap();
        let response = match response {
            FilterChanges::Logs(response) => response,
            _ => panic!("Expected FilterChanges::Logs"),
        };

        assert_eq!(response, rpc_logs());
    }

    #[tokio::test]
    async fn test_get_filter_changes_blocks() {
        let client = get_client().await;
        let filter_id = rpc_filter_id_blocks();

        let response = client.get_filter_changes(filter_id).await.unwrap();
        let response = match response {
            FilterChanges::Hashes(response) => response,
            _ => panic!("Expected FilterChanges::Hashes"),
        };

        assert_eq!(response, rpc_filter_block_hashes());
    }

    #[tokio::test]
    async fn test_get_filter_changes_txs() {
        let client = get_client().await;
        let filter_id = rpc_filter_id_txs();

        let response = client.get_filter_changes(filter_id).await.unwrap();
        let response = match response {
            FilterChanges::Hashes(response) => response,
            _ => panic!("Expected FilterChanges::Hashes"),
        };

        assert_eq!(response, rpc_filter_tx_hashes());
    }

    #[tokio::test]
    async fn test_get_filter_logs() {
        let client = get_client().await;
        let filter_id = rpc_filter_id_logs();

        let response = client.get_filter_logs(filter_id).await.unwrap();

        assert_eq!(response, rpc_logs());
    }

    #[tokio::test]
    async fn test_create_extended_access_list() {
        let client = get_client().await;
        let address = rpc_proof().address;
        let block_beneficiary = rpc_block().header.beneficiary;
        let tx = TransactionRequest::default()
            .from(address)
            .to(block_beneficiary)
            .value(U256::ZERO);

        let response = client
            .create_extended_access_list(&tx, false, BlockId::latest().into())
            .await
            .unwrap();

        assert_eq!(response.len(), 2);
        assert_eq!(response.get(&address).unwrap(), &rpc_account());
        assert_eq!(
            response.get(&block_beneficiary).unwrap(),
            &rpc_block_miner_account()
        );
    }

    #[tokio::test]
    async fn test_chain_id() {
        let client = get_client().await;

        let response = client.chain_id().await.unwrap();

        assert_eq!(response, rpc_chain_id());
    }

    #[tokio::test]
    async fn test_get_block() {
        let client = get_client().await;

        let response = client
            .get_block(BlockId::latest(), false)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(response, rpc_block());
    }

    #[tokio::test]
    async fn test_get_untrusted_block() {
        let client = get_client().await;

        let response = client
            .get_untrusted_block(BlockId::latest(), false)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(response, rpc_block());
    }

    #[tokio::test]
    async fn test_get_block_receipts() {
        let client = get_client().await;

        let response = client
            .get_block_receipts(BlockId::latest())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(response, rpc_block_receipts());
    }

    #[tokio::test]
    async fn test_send_raw_transaction() {
        let client = get_client().await;

        let response = client.send_raw_transaction(&[]).await.unwrap();

        assert_eq!(response, *rpc_tx().inner.tx_hash());
    }

    #[tokio::test]
    async fn test_new_filter() {
        let client = get_client().await;
        let filter = Filter::default();

        let response = client.new_filter(&filter).await.unwrap();

        assert_eq!(response, rpc_filter_id_logs());
    }

    #[tokio::test]
    async fn test_new_block_filter() {
        let client = get_client().await;

        let response = client.new_block_filter().await.unwrap();

        assert_eq!(response, rpc_filter_id_blocks());
    }

    #[tokio::test]
    async fn test_new_pending_transaction_filter() {
        let client = get_client().await;

        let response = client.new_pending_transaction_filter().await.unwrap();

        assert_eq!(response, rpc_filter_id_txs());
    }

    #[tokio::test]
    async fn test_uninstall_filter() {
        let client = get_client().await;

        let response = client.uninstall_filter(rpc_filter_id_logs()).await.unwrap();

        assert_eq!(response, true);
    }
}
