use std::{fs::read_to_string, path::PathBuf, str::FromStr};

use alloy::{
    eips::BlockId,
    network::{ReceiptResponse, TransactionResponse},
    primitives::{Address, B256, U256},
    rpc::types::{EIP1186AccountProofResponse, Filter},
};
use async_trait::async_trait;
use eyre::{eyre, Result};

use helios_common::network_spec::NetworkSpec;
use helios_verifiable_api_types::*;

use super::VerifiableApi;

#[derive(Clone)]
pub struct MockVerifiableApi {
    path: PathBuf,
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec> VerifiableApi<N> for MockVerifiableApi {
    fn new(base_path: &str) -> Self {
        let path = PathBuf::from(base_path);
        Self { path }
    }

    async fn get_account(
        &self,
        address: Address,
        _storage_slots: &[U256],
        _block_id: Option<BlockId>,
        include_code: bool,
    ) -> Result<AccountResponse> {
        let proof: EIP1186AccountProofResponse =
            serde_json::from_str(&read_to_string(self.path.join("rpc/proof.json"))?)?;
        let block_miner_proof: EIP1186AccountProofResponse = serde_json::from_str(
            &read_to_string(self.path.join("rpc/block_miner_proof.json"))?,
        )?;
        let account: AccountResponse =
            serde_json::from_str(&read_to_string(self.path.join("rpc/account.json"))?)?;
        let block_miner_account: AccountResponse = serde_json::from_str(&read_to_string(
            self.path.join("rpc/block_miner_account.json"),
        )?)?;

        let mut account_response = match address {
            address if address == proof.address => Ok(account),
            address if address == block_miner_proof.address => Ok(block_miner_account),
            _ => Err(eyre!("Account not found")),
        }?;

        if !include_code {
            account_response.code = None;
        }
        Ok(account_response)
    }

    async fn get_transaction_receipt(
        &self,
        tx_hash: B256,
    ) -> Result<Option<TransactionReceiptResponse<N>>> {
        let json_str = read_to_string(self.path.join("verifiable_api/receipt.json"))?;
        let receipt: TransactionReceiptResponse<N> = serde_json::from_str(&json_str)?;
        let receipt = if receipt.receipt.transaction_hash() == tx_hash {
            Some(receipt)
        } else {
            None
        };
        Ok(receipt)
    }

    async fn get_logs(&self, _filter: &Filter) -> Result<LogsResponse<N>> {
        let json_str = read_to_string(self.path.join("verifiable_api/logs.json"))?;
        Ok(serde_json::from_str(&json_str)?)
    }

    async fn get_filter_logs(&self, filter_id: U256) -> Result<FilterLogsResponse<N>> {
        let filter_id_logs =
            U256::from_str(&read_to_string(self.path.join("rpc/filter_id_logs.txt"))?)?;
        if filter_id == filter_id_logs {
            let logs = read_to_string(self.path.join("verifiable_api/logs.json"))?;
            Ok(serde_json::from_str(&logs)?)
        } else {
            Err(eyre!("Filter ID not found"))
        }
    }

    async fn get_filter_changes(&self, filter_id: U256) -> Result<FilterChangesResponse<N>> {
        let filter_id_logs =
            U256::from_str(&read_to_string(self.path.join("rpc/filter_id_logs.txt"))?)?;
        let filter_id_blocks =
            U256::from_str(&read_to_string(self.path.join("rpc/filter_id_blocks.txt"))?)?;
        let filter_id_txs =
            U256::from_str(&read_to_string(self.path.join("rpc/filter_id_txs.txt"))?)?;
        match filter_id {
            id if id == filter_id_logs => {
                let logs = read_to_string(self.path.join("verifiable_api/logs.json"))?;
                Ok(FilterChangesResponse::Logs(serde_json::from_str(&logs)?))
            }
            id if id == filter_id_blocks => {
                let hashes = read_to_string(self.path.join("rpc/block_hashes.json"))?;
                Ok(FilterChangesResponse::Hashes(serde_json::from_str(
                    &hashes,
                )?))
            }
            id if id == filter_id_txs => {
                let hashes = read_to_string(self.path.join("rpc/tx_hashes.json"))?;
                Ok(FilterChangesResponse::Hashes(serde_json::from_str(
                    &hashes,
                )?))
            }
            _ => Err(eyre!("Filter ID not found")),
        }
    }

    async fn create_extended_access_list(
        &self,
        _tx: N::TransactionRequest,
        _validate_tx: bool,
        _block_id: Option<BlockId>,
    ) -> Result<ExtendedAccessListResponse> {
        let json_str = read_to_string(self.path.join("verifiable_api/access_list.json"))?;
        Ok(serde_json::from_str(&json_str)?)
    }

    async fn chain_id(&self) -> Result<ChainIdResponse> {
        let json_str = read_to_string(self.path.join("rpc/chain_id.txt"))?;
        Ok(ChainIdResponse {
            chain_id: u64::from_str(&json_str)?,
        })
    }

    async fn get_block(
        &self,
        _block_id: BlockId,
        _full_tx: bool,
    ) -> Result<Option<N::BlockResponse>> {
        let json_str = read_to_string(self.path.join("rpc/block.json"))?;
        let block = serde_json::from_str(&json_str)?;
        Ok(Some(block))
    }

    async fn get_block_receipts(&self, _block: BlockId) -> Result<Option<Vec<N::ReceiptResponse>>> {
        let receipts = read_to_string(self.path.join("rpc/receipts.json"))?;
        Ok(serde_json::from_str(&receipts)?)
    }

    async fn send_raw_transaction(&self, _bytes: &[u8]) -> Result<SendRawTxResponse> {
        let tx = read_to_string(self.path.join("rpc/transaction.json"))?;
        let tx = serde_json::from_str::<N::TransactionResponse>(&tx)?;
        Ok(SendRawTxResponse { hash: tx.tx_hash() })
    }

    async fn new_filter(&self, _filter: &Filter) -> Result<NewFilterResponse> {
        let id = read_to_string(self.path.join("rpc/filter_id_logs.txt"))?;
        Ok(NewFilterResponse {
            id: U256::from_str(&id)?,
            kind: FilterKind::Logs,
        })
    }

    async fn new_block_filter(&self) -> Result<NewFilterResponse> {
        let id = read_to_string(self.path.join("rpc/filter_id_blocks.txt"))?;
        Ok(NewFilterResponse {
            id: U256::from_str(&id)?,
            kind: FilterKind::NewBlocks,
        })
    }

    async fn new_pending_transaction_filter(&self) -> Result<NewFilterResponse> {
        let id = read_to_string(self.path.join("rpc/filter_id_txs.txt"))?;
        Ok(NewFilterResponse {
            id: U256::from_str(&id)?,
            kind: FilterKind::NewPendingTransactions,
        })
    }

    async fn uninstall_filter(&self, filter_id: U256) -> Result<UninstallFilterResponse> {
        let id = read_to_string(self.path.join("rpc/filter_id_logs.txt"))?;
        let id = U256::from_str(&id)?;
        Ok(UninstallFilterResponse {
            ok: id == filter_id,
        })
    }
}
