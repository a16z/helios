use std::{collections::HashMap, future::Future, marker::PhantomData, sync::Arc};

use alloy::{
    eips::{BlockId, BlockNumberOrTag},
    network::{primitives::HeaderResponse, BlockResponse},
};
use eyre::Result;
use revm::{
    database::DatabaseAsync,
    primitives::{address, Address, B256, U256},
    state::{AccountInfo, Bytecode},
};
use tracing::trace;

use helios_common::{
    execution_provider::ExecutionProivder,
    network_spec::NetworkSpec,
    types::{Account, EvmError},
};

use crate::types::DatabaseError;

pub struct ProofDB<N: NetworkSpec, E: ExecutionProivder<N>> {
    pub state: EvmState<N, E>,
}

impl<N: NetworkSpec, E: ExecutionProivder<N>> ProofDB<N, E> {
    pub fn new(block_id: BlockId, execution: Arc<E>) -> Self {
        let state = EvmState::new(execution, block_id);
        ProofDB { state }
    }
}

pub struct EvmState<N: NetworkSpec, E: ExecutionProivder<N>> {
    pub accounts: HashMap<Address, Account>,
    pub block_hash: HashMap<u64, B256>,
    pub block: BlockId,
    pub execution: Arc<E>,
    pub phantom: PhantomData<N>,
}

impl<N: NetworkSpec, E: ExecutionProivder<N>> EvmState<N, E> {
    pub fn new(execution: Arc<E>, block: BlockId) -> Self {
        Self {
            execution,
            block,
            accounts: HashMap::new(),
            block_hash: HashMap::new(),
            phantom: PhantomData,
        }
    }

    pub async fn get_basic_async(
        &mut self,
        address: Address,
    ) -> Result<AccountInfo, DatabaseError> {
        if let Some(account) = self.accounts.get(&address) {
            Ok(AccountInfo::new(
                account.account.balance,
                account.account.nonce,
                account.account.code_hash,
                Bytecode::new_raw(account.code.as_ref().unwrap().clone()),
            ))
        } else {
            // Fetch the account data
            let account = self
                .execution
                .get_account(address, &[], true, self.block)
                .await
                .map_err(|_| DatabaseError::StateMissing)?;

            self.accounts.insert(address, account.clone());

            Ok(AccountInfo::new(
                account.account.balance,
                account.account.nonce,
                account.account.code_hash,
                Bytecode::new_raw(account.code.as_ref().unwrap().clone()),
            ))
        }
    }

    pub async fn get_storage_async(
        &mut self,
        address: Address,
        slot: U256,
    ) -> Result<U256, DatabaseError> {
        if let Some(account) = self.accounts.get(&address) {
            if let Some(value) = account.get_storage_value(B256::from(slot)) {
                return Ok(value);
            }
        }

        // Fetch the storage slot
        let slot_bytes = B256::from(slot);
        let account = self
            .execution
            .get_account(address, &[slot_bytes], true, self.block)
            .await
            .map_err(|_| DatabaseError::StateMissing)?;

        if let Some(stored_account) = self.accounts.get_mut(&address) {
            if stored_account.code.is_none() {
                stored_account.code = account.code;
            }

            for storage_proof in account.storage_proof {
                stored_account.storage_proof.push(storage_proof);
            }
        } else {
            self.accounts.insert(address, account.clone());
        }

        self.accounts
            .get(&address)
            .and_then(|acc| acc.get_storage_value(B256::from(slot)))
            .ok_or(DatabaseError::StateMissing)
    }

    pub async fn get_block_hash_async(&mut self, number: u64) -> Result<B256, DatabaseError> {
        if let Some(hash) = self.block_hash.get(&number) {
            Ok(*hash)
        } else {
            // Fetch the block hash
            let block_id = BlockNumberOrTag::Number(number).into();
            let block = self
                .execution
                .get_block(block_id, false)
                .await
                .map_err(|_| DatabaseError::StateMissing)?
                .ok_or(DatabaseError::StateMissing)?;

            let hash = block.header().hash();
            self.block_hash.insert(number, hash);
            Ok(hash)
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

impl<N: NetworkSpec, E: ExecutionProivder<N>> DatabaseAsync for ProofDB<N, E> {
    type Error = DatabaseError;

    fn basic_async(
        &mut self,
        address: Address,
    ) -> impl Future<Output = Result<Option<AccountInfo>, Self::Error>> + Send {
        async move {
            if is_precompile(&address) {
                return Ok(Some(AccountInfo::default()));
            }

            trace!(
                target: "helios::evm",
                "fetch basic evm state for address=0x{}",
                hex::encode(address.as_slice())
            );

            Ok(Some(self.state.get_basic_async(address).await?))
        }
    }

    fn code_by_hash_async(
        &mut self,
        _code_hash: B256,
    ) -> impl Future<Output = Result<Bytecode, Self::Error>> + Send {
        async move { Err(DatabaseError::Unimplemented) }
    }

    fn storage_async(
        &mut self,
        address: Address,
        slot: U256,
    ) -> impl Future<Output = Result<U256, Self::Error>> + Send {
        async move {
            trace!(target: "helios::evm", "fetch evm state for address={:?}, slot={}", address, slot);
            self.state.get_storage_async(address, slot).await
        }
    }

    fn block_hash_async(
        &mut self,
        number: u64,
    ) -> impl Future<Output = Result<B256, Self::Error>> + Send {
        async move {
            trace!(target: "helios::evm", "fetch block hash for block={:?}", number);
            self.state.get_block_hash_async(number).await
        }
    }
}

fn is_precompile(address: &Address) -> bool {
    address.le(&address!("0000000000000000000000000000000000000009")) && address.gt(&Address::ZERO)
}
