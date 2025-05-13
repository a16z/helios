use std::{collections::HashMap, mem, sync::Arc};

use alloy::{
    network::{primitives::HeaderResponse, BlockResponse},
    rpc::types::{AccessListItem, AccessListResult, EIP1186StorageProof},
};
use eyre::Result;
use revm::{
    context::{result::ExecutionResult, CfgEnv, ContextTr},
    primitives::{address, Address, Bytes, B256, U256},
    state::{AccountInfo, Bytecode},
    Context, Database, ExecuteEvm, MainBuilder, MainContext,
};
use tracing::trace;

use helios_common::{
    fork_schedule::ForkSchedule,
    network_spec::NetworkSpec,
    types::{Account, BlockTag},
};

use super::{
    errors::{DatabaseError, EvmError, ExecutionError},
    spec::ExecutionSpec,
};

pub struct Evm<N: NetworkSpec> {
    execution: Arc<dyn ExecutionSpec<N>>,
    chain_id: u64,
    tag: BlockTag,
    fork_schedule: ForkSchedule,
}

impl<N: NetworkSpec> Evm<N> {
    pub fn new(
        execution: Arc<dyn ExecutionSpec<N>>,
        chain_id: u64,
        fork_schedule: ForkSchedule,
        tag: BlockTag,
    ) -> Self {
        Evm {
            execution,
            chain_id,
            tag,
            fork_schedule,
        }
    }

    pub async fn call(&mut self, tx: &N::TransactionRequest) -> Result<Bytes, EvmError> {
        let (result, ..) = self.call_inner(tx, false).await?;

        match result {
            ExecutionResult::Success { output, .. } => Ok(output.into_data()),
            ExecutionResult::Revert { output, .. } => {
                Err(EvmError::Revert(Some(output.to_vec().into())))
            }
            ExecutionResult::Halt { .. } => Err(EvmError::Revert(None)),
        }
    }

    pub async fn estimate_gas(&mut self, tx: &N::TransactionRequest) -> Result<u64, EvmError> {
        let (result, ..) = self.call_inner(tx, true).await?;

        match result {
            ExecutionResult::Success { gas_used, .. } => Ok(gas_used),
            ExecutionResult::Revert { gas_used, .. } => Ok(gas_used),
            ExecutionResult::Halt { gas_used, .. } => Ok(gas_used),
        }
    }

    pub async fn create_access_list(
        &mut self,
        tx: &N::TransactionRequest,
        validate_tx: bool,
    ) -> Result<AccessListResultWithAccounts, EvmError> {
        let (result, accounts) = self.call_inner(tx, validate_tx).await?;

        Ok(AccessListResultWithAccounts {
            access_list_result: AccessListResult {
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
            },
            accounts,
        })
    }

    async fn call_inner(
        &mut self,
        tx: &N::TransactionRequest,
        validate_tx: bool,
    ) -> Result<(ExecutionResult, HashMap<Address, Account>), EvmError> {
        let mut db = ProofDB::new(self.tag, self.execution.clone());
        _ = db.state.prefetch_state(tx, validate_tx).await;

        let mut evm = self
            .get_context(tx, self.tag, validate_tx)
            .await
            .with_db(db)
            .build_mainnet();

        let tx_res = loop {
            let db = evm.db();
            if db.state.needs_update() {
                db.state.update_state().await.unwrap();
            }

            let res = evm.transact(N::tx_env(tx));

            let db = evm.db();
            let needs_update = db.state.needs_update();

            if res.is_ok() || !needs_update {
                break res.map(|res| (res.result, mem::take(&mut db.state.accounts)));
            }
        };

        tx_res.map_err(|err| EvmError::Generic(format!("generic: {}", err)))
    }

    async fn get_context(
        &self,
        tx: &N::TransactionRequest,
        tag: BlockTag,
        validate_tx: bool,
    ) -> Context {
        let block = self
            .execution
            .get_block(tag.into(), false)
            .await
            .unwrap()
            .ok_or(ExecutionError::BlockNotFound(tag))
            .unwrap();

        let mut cfg = CfgEnv::default();
        cfg.chain_id = self.chain_id;
        cfg.disable_block_gas_limit = !validate_tx;
        cfg.disable_eip3607 = !validate_tx;
        cfg.disable_base_fee = !validate_tx;
        cfg.disable_nonce_check = !validate_tx;

        Context::mainnet()
            .with_tx(N::tx_env(tx))
            .with_block(N::block_env(&block, &self.fork_schedule))
            .with_cfg(cfg)
    }
}

struct ProofDB<N: NetworkSpec> {
    state: EvmState<N>,
}

impl<N: NetworkSpec> ProofDB<N> {
    pub fn new(tag: BlockTag, execution: Arc<dyn ExecutionSpec<N>>) -> Self {
        let state = EvmState::new(execution, tag);
        ProofDB { state }
    }
}

enum StateAccess {
    Basic(Address),
    BlockHash(u64),
    Storage(Address, U256),
}

struct EvmState<N: NetworkSpec> {
    accounts: HashMap<Address, Account>,
    block_hash: HashMap<u64, B256>,
    block: BlockTag,
    access: Option<StateAccess>,
    execution: Arc<dyn ExecutionSpec<N>>,
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
pub struct AccessListResultWithAccounts {
    pub access_list_result: AccessListResult,
    pub accounts: HashMap<Address, Account>,
}

// #[cfg(test)]
// mod tests {
//     use revm::primitives::KECCAK_EMPTY;
//     use tokio::sync::{mpsc::channel, watch};
//
//     use crate::execution::{rpc::mock_rpc::MockRpc, state::State};
//
//     use super::*;
//
//     fn get_client() -> ExecutionClient<Ethereum, MockRpc> {
//         let (_, block_recv) = channel(256);
//         let (_, finalized_recv) = watch::channel(None);
//         let state = State::new(block_recv, finalized_recv, 64);
//         ExecutionClient::new("testdata/", state).unwrap()
//     }
//
//     #[tokio::test]
//     async fn test_proof_db() {
//         // Construct proofdb params
//         let execution = get_client();
//         let tag = BlockTag::Latest;
//
//         // Construct the proof database with the given client
//         let mut proof_db = ProofDB::new(tag, Arc::new(execution));
//
//         let address = address!("388C818CA8B9251b393131C08a736A67ccB19297");
//         let info = AccountInfo::new(
//             U256::from(500),
//             10,
//             KECCAK_EMPTY,
//             Bytecode::new_raw(revm::primitives::Bytes::default()),
//         );
//         proof_db.state.basic.insert(address, info.clone());
//
//         // Get the account from the proof database
//         let account = proof_db.basic(address).unwrap().unwrap();
//
//         assert_eq!(account, info);
//     }
// }
