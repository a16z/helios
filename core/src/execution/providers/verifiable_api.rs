use std::collections::{hash_map::Entry, HashMap, HashSet};

use alloy::{
    consensus::BlockHeader,
    eips::BlockId,
    network::{primitives::HeaderResponse, BlockResponse, ReceiptResponse, TransactionResponse},
    primitives::{Address, B256, U256},
    rlp,
    rpc::types::{EIP1186AccountProofResponse, Filter, Log},
};
use async_trait::async_trait;
use eyre::{eyre, Result};
use url::Url;

use futures::future::try_join_all;
use helios_common::{
    execution_provider::{
        AccountProvider, BlockProvider, ExecutionHintProvider, ExecutionProvider, LogProvider,
        ReceiptProvider, TransactionProvider,
    },
    network_spec::NetworkSpec,
    types::Account,
};
use helios_verifiable_api_client::{
    http::HttpVerifiableApi,
    types::{
        AccountResponse, ExtendedAccessListResponse, LogsResponse, SendRawTxResponse,
        TransactionReceiptResponse,
    },
    VerifiableApi,
};

use crate::execution::{
    errors::ExecutionError,
    proof::{
        verify_account_proof, verify_block_receipts, verify_code_hash_proof, verify_receipt_proof,
        verify_storage_proof, verify_transaction_proof,
    },
};

use super::{historical::HistoricalBlockProvider, utils::ensure_logs_match_filter};

pub struct VerifiableApiExecutionProvider<
    N: NetworkSpec,
    B: BlockProvider<N>,
    H: HistoricalBlockProvider<N>,
> {
    api: HttpVerifiableApi<N>,
    block_provider: B,
    historical_provider: Option<H>,
}

impl<N: NetworkSpec, B: BlockProvider<N>, H: HistoricalBlockProvider<N>> ExecutionProvider<N>
    for VerifiableApiExecutionProvider<N, B, H>
{
}

impl<N: NetworkSpec, B: BlockProvider<N>, H: HistoricalBlockProvider<N>>
    VerifiableApiExecutionProvider<N, B, H>
{
    pub fn new(url: &Url, block_provider: B) -> VerifiableApiExecutionProvider<N, B, ()> {
        VerifiableApiExecutionProvider {
            api: HttpVerifiableApi::new(url),
            block_provider,
            historical_provider: None,
        }
    }

    pub fn with_historical_provider(url: &Url, block_provider: B, historical_provider: H) -> Self {
        Self {
            api: HttpVerifiableApi::new(url),
            block_provider,
            historical_provider: Some(historical_provider),
        }
    }

    fn verify_account(
        &self,
        address: Address,
        account: &AccountResponse,
        block: &N::BlockResponse,
    ) -> Result<()> {
        let proof = EIP1186AccountProofResponse {
            address,
            balance: account.account.balance,
            code_hash: account.account.code_hash,
            nonce: account.account.nonce,
            storage_hash: account.account.storage_root,
            account_proof: account.account_proof.clone(),
            storage_proof: account.storage_proof.clone(),
        };
        // Verify the account proof
        verify_account_proof(&proof, block.header().state_root())?;
        // Verify the storage proofs
        verify_storage_proof(&proof)?;
        // Verify the code hash (if code is included in the response)
        if let Some(code) = &account.code {
            verify_code_hash_proof(&proof, code)?;
        }

        Ok(())
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec, B: BlockProvider<N>, H: HistoricalBlockProvider<N>> AccountProvider<N>
    for VerifiableApiExecutionProvider<N, B, H>
{
    async fn get_account(
        &self,
        address: Address,
        slots: &[B256],
        with_code: bool,
        block_id: BlockId,
    ) -> Result<Account> {
        let block = self
            .get_block(block_id, false)
            .await?
            .ok_or(eyre!("block not found"))?;

        let block_id = BlockId::number(block.header().number());
        let slots = slots.iter().map(|s| (*s).into()).collect::<Vec<U256>>();
        let account = self
            .api
            .get_account(address, &slots, Some(block_id), with_code)
            .await?;

        self.verify_account(address, &account, &block)?;
        Ok(account)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec, B: BlockProvider<N>, H: HistoricalBlockProvider<N>> BlockProvider<N>
    for VerifiableApiExecutionProvider<N, B, H>
{
    async fn push_block(&self, block: <N>::BlockResponse, block_id: BlockId) {
        self.block_provider.push_block(block, block_id).await;
    }

    async fn get_block(
        &self,
        block_id: BlockId,
        full_tx: bool,
    ) -> Result<Option<<N>::BlockResponse>> {
        // 1. Try block cache first
        if let Some(block) = self.block_provider.get_block(block_id, full_tx).await? {
            return Ok(Some(block));
        }

        // 2. Try historical provider if available and only for block numbers or hashes (not tags)
        if let Some(historical) = &self.historical_provider {
            if super::utils::should_use_historical_provider(&block_id) {
                if let Some(block) = historical
                    .get_historical_block(block_id, full_tx, self)
                    .await?
                {
                    // Note: Do NOT cache historical blocks to avoid interfering with consistency detection
                    return Ok(Some(block));
                }
            }
        }

        // 3. No historical block found
        Ok(None)
    }

    async fn get_untrusted_block(
        &self,
        block_id: BlockId,
        full_tx: bool,
    ) -> Result<Option<<N>::BlockResponse>> {
        self.api.get_block(block_id, full_tx).await
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec, B: BlockProvider<N>, H: HistoricalBlockProvider<N>> TransactionProvider<N>
    for VerifiableApiExecutionProvider<N, B, H>
{
    async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<B256> {
        let SendRawTxResponse { hash } = self.api.send_raw_transaction(bytes).await?;
        Ok(hash)
    }

    async fn get_transaction(&self, hash: B256) -> Result<Option<<N>::TransactionResponse>> {
        let Some(tx_res) = self.api.get_transaction(hash).await? else {
            return Ok(None);
        };

        let tx = tx_res.transaction;
        let proof = tx_res.transaction_proof;

        let Some(block_hash) = tx.block_hash() else {
            return Ok(None);
        };

        let transactions_root = self
            .get_block(block_hash.into(), false)
            .await?
            .ok_or(eyre!("block not found"))?
            .header()
            .transactions_root();

        verify_transaction_proof::<N>(&tx, transactions_root, &proof)?;
        Ok(Some(tx))
    }

    async fn get_transaction_by_location(
        &self,
        block_id: BlockId,
        index: u64,
    ) -> Result<Option<<N>::TransactionResponse>> {
        let block = self
            .get_block(block_id, false)
            .await?
            .ok_or(eyre!("block not found"))?;

        let block_id = block.header().hash().into();
        let Some(tx_res) = self
            .api
            .get_transaction_by_location(block_id, index)
            .await?
        else {
            return Ok(None);
        };

        let tx = tx_res.transaction;
        let proof = tx_res.transaction_proof;

        let tx_index = tx
            .transaction_index()
            .ok_or(eyre!("transaction not included"))?;

        if tx_index != index {
            return Err(eyre!("tx index mismatch"));
        }

        let transactions_root = block.header().transactions_root();
        verify_transaction_proof::<N>(&tx, transactions_root, &proof)?;
        Ok(Some(tx))
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec, B: BlockProvider<N>, H: HistoricalBlockProvider<N>> ReceiptProvider<N>
    for VerifiableApiExecutionProvider<N, B, H>
{
    async fn get_receipt(&self, hash: B256) -> Result<Option<N::ReceiptResponse>> {
        let Some(receipt_response) = self.api.get_transaction_receipt(hash).await? else {
            return Ok(None);
        };

        let Some(block_hash) = receipt_response.receipt.block_hash() else {
            return Ok(None);
        };

        let receipts_root = self
            .get_block(block_hash.into(), false)
            .await?
            .ok_or(eyre!("block not found"))?
            .header()
            .receipts_root();

        verify_receipt_proof::<N>(
            &receipt_response.receipt,
            receipts_root,
            &receipt_response.receipt_proof,
        )?;

        Ok(Some(receipt_response.receipt))
    }

    async fn get_block_receipts(
        &self,
        block_id: BlockId,
    ) -> Result<Option<Vec<N::ReceiptResponse>>> {
        let Some(block) = self.get_block(block_id, false).await? else {
            return Ok(None);
        };

        let block_num = block.header().number();

        let receipts = self
            .api
            .get_block_receipts(block_num.into())
            .await?
            .ok_or(ExecutionError::NoReceiptsForBlock(block_num.into()))?;

        verify_block_receipts::<N>(&receipts, &block)?;

        Ok(Some(receipts))
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec, B: BlockProvider<N>, H: HistoricalBlockProvider<N>> LogProvider<N>
    for VerifiableApiExecutionProvider<N, B, H>
{
    async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>> {
        let LogsResponse {
            logs,
            receipt_proofs,
        } = self.api.get_logs(filter).await?;

        // Map of tx_hash -> encoded receipt logs to avoid encoding multiple times
        let mut txhash_encodedlogs_map: HashMap<B256, Vec<Vec<u8>>> = HashMap::new();

        // Verify each log entry exists in the corresponding receipt logs
        for log in &logs {
            let tx_hash = log.transaction_hash.unwrap();
            let log_encoded = rlp::encode(&log.inner);

            if let Entry::Vacant(e) = txhash_encodedlogs_map.entry(tx_hash) {
                let TransactionReceiptResponse {
                    receipt,
                    receipt_proof: _,
                } = receipt_proofs
                    .get(&tx_hash)
                    .ok_or(ExecutionError::NoReceiptForTransaction(tx_hash))?;

                let encoded_logs = N::receipt_logs(receipt)
                    .iter()
                    .map(|l| rlp::encode(&l.inner))
                    .collect::<Vec<_>>();
                e.insert(encoded_logs);
            }
            let receipt_logs_encoded = txhash_encodedlogs_map.get(&tx_hash).unwrap();

            if !receipt_logs_encoded.contains(&log_encoded) {
                return Err(ExecutionError::MissingLog(
                    tx_hash,
                    U256::from(log.log_index.unwrap()),
                )
                .into());
            }
        }

        // fetch required blocks
        let mut blocks_required = HashSet::new();
        for receipt_proof in &receipt_proofs {
            let block_hash = receipt_proof.1.receipt.block_hash().unwrap();
            blocks_required.insert(block_hash);
        }

        let receipts_roots_fut = blocks_required.iter().map(async |block_hash| {
            let root = self
                .get_block((*block_hash).into(), false)
                .await?
                .ok_or(eyre!("block not found"))?
                .header()
                .receipts_root();

            Ok::<_, eyre::Report>((*block_hash, root))
        });

        let receipts_root_vec = try_join_all(receipts_roots_fut).await?;
        let mut receipts_roots = HashMap::new();
        for (block_hash, receipts_root) in receipts_root_vec {
            receipts_roots.insert(block_hash, receipts_root);
        }

        // Verify all receipts
        for receipt_proof in receipt_proofs {
            let (_, receipt_response) = receipt_proof;
            let receipt = &receipt_response.receipt;
            let proof = &receipt_response.receipt_proof;

            let block_hash = receipt.block_hash().unwrap();
            let receipts_root = receipts_roots.get(&block_hash).unwrap();

            verify_receipt_proof::<N>(receipt, *receipts_root, proof)?;
        }

        ensure_logs_match_filter(&logs, filter)?;
        Ok(logs)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec, B: BlockProvider<N>, H: HistoricalBlockProvider<N>> ExecutionHintProvider<N>
    for VerifiableApiExecutionProvider<N, B, H>
{
    async fn get_execution_hint(
        &self,
        tx: &<N>::TransactionRequest,
        validate: bool,
        block_id: BlockId,
    ) -> Result<HashMap<Address, Account>> {
        let block = self
            .get_block(block_id, false)
            .await?
            .ok_or(eyre!("block not found"))?;

        let block_id = block.header().hash().into();
        let ExtendedAccessListResponse { accounts } = self
            .api
            .get_execution_hint(tx.clone(), validate, Some(block_id))
            .await?;

        for (address, account) in &accounts {
            self.verify_account(*address, account, &block)?;
        }

        Ok(accounts)
    }
}
