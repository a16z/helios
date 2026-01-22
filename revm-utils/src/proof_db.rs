use std::{collections::HashMap, marker::PhantomData, sync::Arc};

use alloy::{
    eips::{eip1898::RpcBlockHash, BlockNumberOrTag},
    network::{primitives::HeaderResponse, BlockResponse},
    rpc::types::state::{AccountOverride, StateOverride},
};
use eyre::Result;
use revm::{
    primitives::{address, Address, B256, KECCAK_EMPTY, U256},
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
    pub fn new(
        block: RpcBlockHash,
        execution: Arc<E>,
        state_overrides: Option<StateOverride>,
    ) -> Self {
        let state = EvmState::new(execution, block, state_overrides);
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
    pub block: RpcBlockHash,
    pub access: Option<StateAccess>,
    pub execution: Arc<E>,
    pub state_overrides: Option<StateOverride>,
    pub phantom: PhantomData<N>,
}

impl<N: NetworkSpec, E: ExecutionProvider<N>> EvmState<N, E> {
    pub fn new(
        execution: Arc<E>,
        block: RpcBlockHash,
        state_overrides: Option<StateOverride>,
    ) -> Self {
        Self {
            execution,
            block,
            accounts: HashMap::new(),
            block_hash: HashMap::new(),
            access: None,
            state_overrides,
            phantom: PhantomData,
        }
    }

    pub async fn update_state(&mut self) -> Result<()> {
        if let Some(access) = self.access.take() {
            match access {
                StateAccess::Basic(address) => {
                    let account = self
                        .execution
                        .get_account(address, &[], true, self.block.into())
                        .await?;

                    self.accounts.insert(address, account);
                }
                StateAccess::Storage(address, slot) => {
                    let slot_bytes = B256::from(slot);
                    let account = self
                        .execution
                        .get_account(address, &[slot_bytes], false, self.block.into())
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
        let override_opt = self
            .state_overrides
            .as_ref()
            .and_then(|overrides| overrides.get(&address));

        if let Some(account) = self.accounts.get_mut(&address) {
            let code_is_overriden = override_opt.and_then(|o| o.code.as_ref()).is_some();
            if account.code.is_none() && !code_is_overriden {
                self.access = Some(StateAccess::Basic(address));
                return Err(DatabaseError::StateMissing);
            }

            if let Some(override_opt) = override_opt {
                apply_account_overrides(account, override_opt)?;
            }

            // Normalize code_hash for REVM compatibility:
            // RPC response for getProof method for non-existing (unused) EOAs
            // may contain B256::ZERO for code_hash, but REVM expects KECCAK_EMPTY
            let code_hash = if account.account.code_hash == B256::ZERO {
                KECCAK_EMPTY
            } else {
                account.account.code_hash
            };

            Ok(AccountInfo::new(
                account.account.balance,
                account.account.nonce,
                code_hash,
                Bytecode::new_raw(account.code.as_ref().unwrap().clone()),
            ))
        } else if override_opt.is_some() {
            // It means we have an override but no actual account data to fall through to.
            // We return error so that the account is fetched first, overrides will be applied on the next iteration.
            self.access = Some(StateAccess::Basic(address));
            Err(DatabaseError::StateMissing)
        } else {
            self.access = Some(StateAccess::Basic(address));
            Err(DatabaseError::StateMissing)
        }
    }

    pub fn get_storage(&mut self, address: Address, slot: U256) -> Result<U256, DatabaseError> {
        let override_opt = self
            .state_overrides
            .as_ref()
            .and_then(|overrides| overrides.get(&address));

        let slot_b256 = B256::from(slot);

        if let Some(account_override) = override_opt {
            if let Some(result) = apply_storage_overrides(&slot_b256, account_override)? {
                return Ok(result);
            }
        }

        if let Some(account) = self.accounts.get(&address) {
            if let Some(value) = account.get_storage_value(slot_b256) {
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
            .get_execution_hint(tx, validate_tx, self.block.into())
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

fn apply_account_overrides(
    account: &mut Account,
    overrides: &AccountOverride,
) -> Result<(), DatabaseError> {
    let balance = overrides.balance.unwrap_or(account.account.balance);

    let nonce = overrides.nonce.unwrap_or(account.account.nonce);

    let (code_hash, code) = if let Some(override_code) = overrides.code.as_ref() {
        let code = Bytecode::new_raw_checked(override_code.clone())
            .map_err(|e| DatabaseError::InvalidStateOverride(e.to_string()))?;
        (code.hash_slow(), code.bytes())
    } else {
        (
            account.account.code_hash,
            account.code.as_ref().unwrap().clone(),
        )
    };
    account.account.balance = balance;
    account.account.nonce = nonce;
    account.account.code_hash = code_hash;
    account.code = Some(code);
    Ok(())
}

fn apply_storage_overrides(
    slot: &B256,
    overrides: &AccountOverride,
) -> Result<Option<U256>, DatabaseError> {
    if overrides.state.is_some() && overrides.state_diff.is_some() {
        return Err(DatabaseError::InvalidStateOverride(
            "Both 'state' and 'stateDiff' defined for account".to_string(),
        ));
    }

    if let Some(ref state) = overrides.state {
        // Full state replacement - only use override values
        let value = state
            .get(slot)
            .map(|b| U256::from_be_bytes(b.0))
            .unwrap_or(U256::ZERO);
        return Ok(Some(value));
    }

    if let Some(ref state_diff) = overrides.state_diff {
        if let Some(value_b256) = state_diff.get(slot) {
            return Ok(Some(U256::from_be_bytes(value_b256.0)));
        }
    }

    // No override applies, fall through to regular storage
    Ok(None)
}
