use std::{borrow::BorrowMut, collections::HashMap, sync::Arc};

use alloy::{network::TransactionBuilder, rpc::types::TransactionRequest};
use eyre::{Report, Result};
use futures::future::join_all;
use revm::{
    primitives::{
        address, AccessListItem, AccountInfo, Address, BlobExcessGasAndPrice, Bytecode, Env,
        ExecutionResult, ResultAndState, B256, U256,
    },
    Database, Evm as Revm,
};
use tracing::trace;

use crate::{
    constants::PARALLEL_QUERY_BATCH_SIZE, errors::EvmError, rpc::ExecutionRpc, ExecutionClient,
};
use common::types::BlockTag;

pub struct Evm<R: ExecutionRpc> {
    execution: Arc<ExecutionClient<R>>,
    chain_id: u64,
    tag: BlockTag,
}

impl<R: ExecutionRpc> Evm<R> {
    pub fn new(execution: Arc<ExecutionClient<R>>, chain_id: u64, tag: BlockTag) -> Self {
        Evm {
            execution,
            chain_id,
            tag,
        }
    }

    pub async fn call(&mut self, tx: &TransactionRequest) -> Result<Vec<u8>, EvmError> {
        let tx = self.call_inner(tx).await?;

        match tx.result {
            ExecutionResult::Success { output, .. } => Ok(output.into_data().to_vec()),
            ExecutionResult::Revert { output, .. } => {
                Err(EvmError::Revert(Some(output.to_vec().into())))
            }
            ExecutionResult::Halt { .. } => Err(EvmError::Revert(None)),
        }
    }

    pub async fn estimate_gas(&mut self, tx: &TransactionRequest) -> Result<u64, EvmError> {
        let tx = self.call_inner(tx).await?;

        match tx.result {
            ExecutionResult::Success { gas_used, .. } => Ok(gas_used),
            ExecutionResult::Revert { gas_used, .. } => Ok(gas_used),
            ExecutionResult::Halt { gas_used, .. } => Ok(gas_used),
        }
    }

    async fn call_inner(&mut self, tx: &TransactionRequest) -> Result<ResultAndState, EvmError> {
        let mut db = ProofDB::new(self.tag, self.execution.clone());
        _ = db.state.prefetch_state(tx).await;

        let env = Box::new(self.get_env(tx, self.tag).await);
        let evm = Revm::builder().with_db(db).with_env(env).build();
        let mut ctx = evm.into_context_with_handler_cfg();

        let tx_res = loop {
            let db = ctx.context.evm.db.borrow_mut();
            if db.state.needs_update() {
                db.state.update_state().await.unwrap();
            }

            let mut evm = Revm::builder().with_context_with_handler_cfg(ctx).build();
            let res = evm.transact();
            ctx = evm.into_context_with_handler_cfg();

            let db = ctx.context.evm.db.borrow_mut();
            let needs_update = db.state.needs_update();

            if res.is_ok() || !needs_update {
                break res;
            }
        };

        tx_res.map_err(|_| EvmError::Generic("evm error".to_string()))
    }

    async fn get_env(&self, tx: &TransactionRequest, tag: BlockTag) -> Env {
        let mut env = Env::default();

        env.tx.caller = tx.from.unwrap_or_default();
        env.tx.gas_limit = tx.gas_limit().map(|v| v as u64).unwrap_or(u64::MAX);
        env.tx.gas_price = tx.gas_price().map(U256::from).unwrap_or_default();
        env.tx.transact_to = tx.to.unwrap_or_default();
        env.tx.value = tx.value.unwrap_or_default();
        env.tx.data = tx.input().unwrap_or_default().clone();
        env.tx.nonce = tx.nonce();
        env.tx.chain_id = tx.chain_id();
        env.tx.access_list = tx.access_list().map(|v| v.to_vec()).unwrap_or_default();
        env.tx.gas_priority_fee = tx.max_priority_fee_per_gas().map(U256::from);
        env.tx.max_fee_per_blob_gas = tx.max_fee_per_gas().map(U256::from);
        env.tx.blob_hashes = tx
            .blob_versioned_hashes
            .as_ref()
            .map(|v| v.to_vec())
            .unwrap_or_default();

        let block = self.execution.get_block(tag, false).await.unwrap();

        env.block.number = block.number.to();
        env.block.coinbase = block.miner;
        env.block.timestamp = block.timestamp.to();
        env.block.gas_limit = block.gas_limit.to();
        env.block.basefee = block.base_fee_per_gas;
        env.block.difficulty = block.difficulty;
        env.block.prevrandao = Some(block.mix_hash);
        env.block.blob_excess_gas_and_price = block
            .excess_blob_gas
            .map(|v| BlobExcessGasAndPrice::new(v.to()));

        env.cfg.chain_id = self.chain_id;
        env.cfg.disable_block_gas_limit = true;
        env.cfg.disable_eip3607 = true;
        env.cfg.disable_base_fee = true;

        env
    }
}

struct ProofDB<R: ExecutionRpc> {
    state: EvmState<R>,
}

impl<R: ExecutionRpc> ProofDB<R> {
    pub fn new(tag: BlockTag, execution: Arc<ExecutionClient<R>>) -> Self {
        let state = EvmState::new(execution.clone(), tag);
        ProofDB { state }
    }
}

enum StateAccess {
    Basic(Address),
    BlockHash(u64),
    Storage(Address, U256),
}

struct EvmState<R: ExecutionRpc> {
    basic: HashMap<Address, AccountInfo>,
    block_hash: HashMap<u64, B256>,
    storage: HashMap<Address, HashMap<U256, U256>>,
    block: BlockTag,
    access: Option<StateAccess>,
    execution: Arc<ExecutionClient<R>>,
}

impl<R: ExecutionRpc> EvmState<R> {
    pub fn new(execution: Arc<ExecutionClient<R>>, block: BlockTag) -> Self {
        Self {
            execution,
            block,
            basic: HashMap::new(),
            storage: HashMap::new(),
            block_hash: HashMap::new(),
            access: None,
        }
    }

    pub async fn update_state(&mut self) -> Result<()> {
        if let Some(access) = &self.access.take() {
            match access {
                StateAccess::Basic(address) => {
                    let account = self
                        .execution
                        .get_account(address, None, self.block)
                        .await?;

                    self.basic.insert(
                        *address,
                        AccountInfo::new(
                            account.balance,
                            account.nonce,
                            account.code_hash,
                            Bytecode::new_raw(account.code.into()),
                        ),
                    );
                }
                StateAccess::Storage(address, slot) => {
                    let slot_bytes = B256::from(*slot);
                    let account = self
                        .execution
                        .get_account(address, Some(&[slot_bytes]), self.block)
                        .await?;

                    let storage = self.storage.entry(*address).or_default();
                    let value = *account.slots.get(&slot_bytes).unwrap();
                    storage.insert(*slot, value);
                }
                StateAccess::BlockHash(number) => {
                    let block = self
                        .execution
                        .get_block(BlockTag::Number(*number), false)
                        .await?;

                    self.block_hash.insert(*number, block.hash);
                }
            }
        }

        Ok(())
    }

    pub fn needs_update(&self) -> bool {
        self.access.is_some()
    }

    pub fn get_basic(&mut self, address: Address) -> Result<AccountInfo> {
        if let Some(account) = self.basic.get(&address) {
            Ok(account.clone())
        } else {
            self.access = Some(StateAccess::Basic(address));
            eyre::bail!("state missing");
        }
    }

    pub fn get_storage(&mut self, address: Address, slot: U256) -> Result<U256> {
        let storage = self.storage.entry(address).or_default();
        if let Some(slot) = storage.get(&slot) {
            Ok(*slot)
        } else {
            self.access = Some(StateAccess::Storage(address, slot));
            eyre::bail!("state missing");
        }
    }

    pub fn get_block_hash(&mut self, block: u64) -> Result<B256> {
        if let Some(hash) = self.block_hash.get(&block) {
            Ok(*hash)
        } else {
            self.access = Some(StateAccess::BlockHash(block));
            eyre::bail!("state missing");
        }
    }

    pub async fn prefetch_state(&mut self, tx: &TransactionRequest) -> Result<()> {
        let mut list = self
            .execution
            .rpc
            .create_access_list(tx, self.block)
            .await
            .map_err(EvmError::RpcError)?
            .0;

        let from_access_entry = AccessListItem {
            address: tx.from.unwrap_or_default(),
            storage_keys: Vec::default(),
        };

        let to_access_entry = AccessListItem {
            address: tx.to().unwrap_or_default(),
            storage_keys: Vec::default(),
        };

        let coinbase = self.execution.get_block(self.block, false).await?.miner;
        let producer_access_entry = AccessListItem {
            address: coinbase,
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
                let account_fut = self.execution.get_account(
                    &account.address,
                    Some(account.storage_keys.as_slice()),
                    self.block,
                );
                async move { (account.address, account_fut.await) }
            });

            let account_chunk = join_all(account_chunk_futs).await;

            account_chunk
                .into_iter()
                .filter(|i| i.1.is_ok())
                .for_each(|(key, value)| {
                    account_map.insert(key, value.ok().unwrap());
                });
        }

        for (address, account) in account_map {
            self.basic.insert(
                address,
                AccountInfo::new(
                    account.balance,
                    account.nonce,
                    account.code_hash,
                    Bytecode::new_raw(account.code.into()),
                ),
            );

            for (slot, value) in account.slots {
                self.storage
                    .entry(address)
                    .or_default()
                    .insert(slot.into(), value);
            }
        }

        Ok(())
    }
}

impl<R: ExecutionRpc> Database for ProofDB<R> {
    type Error = Report;

    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Report> {
        if is_precompile(&address) {
            return Ok(Some(AccountInfo::default()));
        }

        trace!(
            target: "helios::evm",
            "fetch basic evm state for address=0x{}",
            hex::encode(address.as_slice())
        );

        Ok(Some(self.state.get_basic(address)?))
    }

    fn block_hash(&mut self, number: u64) -> Result<B256, Report> {
        trace!(target: "helios::evm", "fetch block hash for block={:?}", number);
        self.state.get_block_hash(number)
    }

    fn storage(&mut self, address: Address, slot: U256) -> Result<U256, Report> {
        trace!(target: "helios::evm", "fetch evm state for address={:?}, slot={}", address, slot);
        self.state.get_storage(address, slot)
    }

    fn code_by_hash(&mut self, _code_hash: B256) -> Result<Bytecode, Report> {
        Err(eyre::eyre!("should never be called"))
    }
}

fn is_precompile(address: &Address) -> bool {
    address.le(&address!("0000000000000000000000000000000000000009")) && address.gt(&Address::ZERO)
}

#[cfg(test)]
mod tests {
    use revm::primitives::KECCAK_EMPTY;
    use tokio::sync::{mpsc::channel, watch};

    use crate::{rpc::mock_rpc::MockRpc, state::State};

    use super::*;

    fn get_client() -> ExecutionClient<MockRpc> {
        let (_, block_recv) = channel(256);
        let (_, finalized_recv) = watch::channel(None);
        let state = State::new(block_recv, finalized_recv, 64);
        ExecutionClient::new("testdata/", state).unwrap()
    }

    #[tokio::test]
    async fn test_proof_db() {
        // Construct proofdb params
        let execution = get_client();
        let tag = BlockTag::Latest;

        // Construct the proof database with the given client
        let mut proof_db = ProofDB::new(tag, Arc::new(execution));

        let address = address!("388C818CA8B9251b393131C08a736A67ccB19297");
        let info = AccountInfo::new(
            U256::from(500),
            10,
            KECCAK_EMPTY,
            Bytecode::new_raw(revm::primitives::Bytes::default()),
        );
        proof_db.state.basic.insert(address, info.clone());

        // Get the account from the proof database
        let account = proof_db.basic(address).unwrap().unwrap();

        assert_eq!(account, info);
    }
}
