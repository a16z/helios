use std::{collections::HashMap, marker::PhantomData, sync::Arc};

use alloy::{
    eips::{BlockId, BlockNumberOrTag},
    network::{primitives::HeaderResponse, BlockResponse},
};
use eyre::Result;
use revm::{
    primitives::{address, Address, B256, U256},
    state::{AccountInfo, Bytecode},
    Database,
};
use tracing::trace;

use helios_common::{
    execution_provider::ExecutionProvider,
    network_spec::NetworkSpec,
    types::{Account, EvmError},
};
use helios_core::execution::errors::ExecutionError;

use crate::types::DatabaseError;

pub struct ProofDB<N: NetworkSpec, E: ExecutionProvider<N>> {
    pub state: EvmState<N, E>,
}

impl<N: NetworkSpec, E: ExecutionProvider<N>> ProofDB<N, E> {
    pub fn new(block_id: BlockId, execution: Arc<E>) -> Self {
        let state = EvmState::new(execution, block_id);
        ProofDB { state }
    }
}

#[derive(Debug)]
pub enum StateAccess {
    Basic(Address),
    BlockHash(u64),
    Storage(Address, U256),
}

pub struct EvmState<N: NetworkSpec, E: ExecutionProvider<N>> {
    pub accounts: HashMap<Address, Account>,
    pub block_hash: HashMap<u64, B256>,
    pub block: BlockId,
    pub access: Option<StateAccess>,
    pub execution: Arc<E>,
    pub phantom: PhantomData<N>,
}

impl<N: NetworkSpec, E: ExecutionProvider<N>> EvmState<N, E> {
    pub fn new(execution: Arc<E>, block: BlockId) -> Self {
        Self {
            execution,
            block,
            accounts: HashMap::new(),
            block_hash: HashMap::new(),
            access: None,
            phantom: PhantomData,
        }
    }

    pub async fn update_state(&mut self) -> Result<()> {
        if let Some(access) = self.access.take() {
            match access {
                StateAccess::Basic(address) => {
                    let account = self
                        .execution
                        .get_account(address, &[], true, self.block)
                        .await?;

                    self.accounts.insert(address, account);
                }
                StateAccess::Storage(address, slot) => {
                    let slot_bytes = B256::from(slot);
                    let account = self
                        .execution
                        .get_account(address, &[slot_bytes], true, self.block)
                        .await?;

                    if let Some(stored_account) = self.accounts.get_mut(&address) {
                        if stored_account.code.is_none() {
                            stored_account.code = account.code;
                        }

                        for storage_proof in account.storage_proof {
                            stored_account.storage_proof.push(storage_proof);
                        }
                    } else {
                        self.accounts.insert(address, account);
                    }
                }
                StateAccess::BlockHash(number) => {
                    let block_id = BlockNumberOrTag::Number(number).into();
                    let block = self
                        .execution
                        .get_block(block_id, false)
                        .await?
                        .ok_or(ExecutionError::BlockNotFound(block_id))?;

                    self.block_hash.insert(number, block.header().hash());
                }
            }
        }

        Ok(())
    }

    pub fn needs_update(&self) -> bool {
        self.access.is_some()
    }

    pub fn get_basic(&mut self, address: Address) -> Result<AccountInfo, DatabaseError> {
        if let Some(account) = self.accounts.get(&address) {
            Ok(AccountInfo::new(
                account.account.balance,
                account.account.nonce,
                account.account.code_hash,
                Bytecode::new_raw(account.code.as_ref().unwrap().clone()),
            ))
        } else {
            self.access = Some(StateAccess::Basic(address));
            Err(DatabaseError::StateMissing)
        }
    }

    pub fn get_storage(&mut self, address: Address, slot: U256) -> Result<U256, DatabaseError> {
        if let Some(account) = self.accounts.get(&address) {
            if let Some(value) = account.get_storage_value(B256::from(slot)) {
                return Ok(value);
            }
        }
        self.access = Some(StateAccess::Storage(address, slot));
        Err(DatabaseError::StateMissing)
    }

    pub fn get_block_hash(&mut self, block: u64) -> Result<B256, DatabaseError> {
        if let Some(hash) = self.block_hash.get(&block) {
            Ok(*hash)
        } else {
            self.access = Some(StateAccess::BlockHash(block));
            Err(DatabaseError::StateMissing)
        }
    }

    pub async fn prefetch_state(
        &mut self,
        tx: &N::TransactionRequest,
        validate_tx: bool,
    ) -> Result<()> {
        let account_map = self
            .execution
            .get_execution_hint(tx, validate_tx, self.block)
            .await
            .map_err(EvmError::RpcError)?;

        for (address, account) in account_map {
            self.accounts.insert(address, account);
        }

        Ok(())
    }
}

impl<N: NetworkSpec, E: ExecutionProvider<N>> Database for ProofDB<N, E> {
    type Error = DatabaseError;

    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, DatabaseError> {
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

    fn block_hash(&mut self, number: u64) -> Result<B256, DatabaseError> {
        trace!(target: "helios::evm", "fetch block hash for block={:?}", number);
        self.state.get_block_hash(number)
    }

    fn storage(&mut self, address: Address, slot: U256) -> Result<U256, DatabaseError> {
        trace!(target: "helios::evm", "fetch evm state for address={:?}, slot={}", address, slot);
        self.state.get_storage(address, slot)
    }

    fn code_by_hash(&mut self, _code_hash: B256) -> Result<Bytecode, DatabaseError> {
        Err(DatabaseError::Unimplemented)
    }
}

fn is_precompile(address: &Address) -> bool {
    address.le(&address!("0000000000000000000000000000000000000009")) && address.gt(&Address::ZERO)
}
