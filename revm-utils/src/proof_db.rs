use std::{collections::HashMap, sync::Arc};

use alloy::network::{primitives::HeaderResponse, BlockResponse};
use eyre::{Report, Result};
use revm::{
    primitives::{address, AccountInfo, Address, Bytecode, B256, U256},
    Database,
};
use tracing::trace;

use helios_common::{
    execution_spec::ExecutionSpec,
    network_spec::NetworkSpec,
    types::{Account, BlockTag, EvmError},
};
use helios_core::execution::errors::ExecutionError;

pub struct ProofDB<N: NetworkSpec> {
    pub state: EvmState<N>,
}

impl<N: NetworkSpec> ProofDB<N> {
    pub fn new(tag: BlockTag, execution: Arc<dyn ExecutionSpec<N>>) -> Self {
        let state = EvmState::new(execution, tag);
        ProofDB { state }
    }
}

pub enum StateAccess {
    Basic(Address),
    BlockHash(u64),
    Storage(Address, U256),
}

pub struct EvmState<N: NetworkSpec> {
    pub accounts: HashMap<Address, Account>,
    pub block_hash: HashMap<u64, B256>,
    pub block: BlockTag,
    pub access: Option<StateAccess>,
    pub execution: Arc<dyn ExecutionSpec<N>>,
}

impl<N: NetworkSpec> EvmState<N> {
    pub fn new(execution: Arc<dyn ExecutionSpec<N>>, block: BlockTag) -> Self {
        Self {
            execution,
            block,
            accounts: HashMap::new(),
            block_hash: HashMap::new(),
            access: None,
        }
    }

    pub async fn update_state(&mut self) -> Result<()> {
        if let Some(access) = self.access.take() {
            match access {
                StateAccess::Basic(address) => {
                    let account = self
                        .execution
                        .get_account(address, None, self.block, true)
                        .await?;

                    self.accounts.insert(address, account);
                }
                StateAccess::Storage(address, slot) => {
                    let slot_bytes = B256::from(slot);
                    let account = self
                        .execution
                        .get_account(address, Some(&[slot_bytes]), self.block, true)
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
                    let tag = BlockTag::Number(number);
                    let block = self
                        .execution
                        .get_block(tag.into(), false)
                        .await?
                        .ok_or(ExecutionError::BlockNotFound(tag))?;

                    self.block_hash.insert(number, block.header().hash());
                }
            }
        }

        Ok(())
    }

    pub fn needs_update(&self) -> bool {
        self.access.is_some()
    }

    pub fn get_basic(&mut self, address: Address) -> Result<AccountInfo> {
        if let Some(account) = self.accounts.get(&address) {
            Ok(AccountInfo::new(
                account.account.balance,
                account.account.nonce,
                account.account.code_hash,
                Bytecode::new_raw(account.code.as_ref().unwrap().clone()),
            ))
        } else {
            self.access = Some(StateAccess::Basic(address));
            eyre::bail!("state missing");
        }
    }

    pub fn get_storage(&mut self, address: Address, slot: U256) -> Result<U256> {
        if let Some(account) = self.accounts.get(&address) {
            if let Some(value) = account.get_storage_value(B256::from(slot)) {
                return Ok(value);
            }
        }
        self.access = Some(StateAccess::Storage(address, slot));
        eyre::bail!("state missing");
    }

    pub fn get_block_hash(&mut self, block: u64) -> Result<B256> {
        if let Some(hash) = self.block_hash.get(&block) {
            Ok(*hash)
        } else {
            self.access = Some(StateAccess::BlockHash(block));
            eyre::bail!("state missing");
        }
    }

    pub async fn prefetch_state(
        &mut self,
        tx: &N::TransactionRequest,
        validate_tx: bool,
    ) -> Result<()> {
        let block_id = Some(self.block.into());
        let account_map = self
            .execution
            .create_extended_access_list(tx, validate_tx, block_id)
            .await
            .map_err(EvmError::RpcError)?;

        for (address, account) in account_map {
            self.accounts.insert(address, account);
        }

        Ok(())
    }
}

impl<N: NetworkSpec> Database for ProofDB<N> {
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
