use std::{collections::HashMap, marker::PhantomData, mem, sync::Arc};

use alloy::{
    eips::{BlockId, BlockNumberOrTag},
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

use helios_common::{fork_schedule::ForkSchedule, network_spec::NetworkSpec, types::Account};

use super::{
    errors::{DatabaseError, EvmError, ExecutionError},
    providers::ExecutionProivder,
};

pub struct Evm<N: NetworkSpec, E: ExecutionProivder<N>> {
    execution: Arc<E>,
    chain_id: u64,
    block_id: BlockId,
    fork_schedule: ForkSchedule,
    phantom: PhantomData<N>,
}

impl<N: NetworkSpec, E: ExecutionProivder<N>> Evm<N, E> {
    pub fn new(
        execution: Arc<E>,
        chain_id: u64,
        fork_schedule: ForkSchedule,
        block_id: BlockId,
    ) -> Self {
        Evm {
            execution,
            chain_id,
            block_id,
            fork_schedule,
            phantom: PhantomData,
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
        let mut db = ProofDB::new(self.block_id, self.execution.clone());
        _ = db.state.prefetch_state(tx, validate_tx).await;

        let mut evm = self
            .get_context(tx, self.block_id, validate_tx)
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
        block_id: BlockId,
        validate_tx: bool,
    ) -> Context {
        let block = self
            .execution
            .get_block(block_id, false)
            .await
            .unwrap()
            .ok_or(ExecutionError::BlockNotFound(block_id))
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

struct ProofDB<N: NetworkSpec, E: ExecutionProivder<N>> {
    state: EvmState<N, E>,
}

impl<N: NetworkSpec, E: ExecutionProivder<N>> ProofDB<N, E> {
    pub fn new(block_id: BlockId, execution: Arc<E>) -> Self {
        let state = EvmState::new(execution, block_id);
        ProofDB { state }
    }
}

enum StateAccess {
    Basic(Address),
    BlockHash(u64),
    Storage(Address, U256),
}

struct EvmState<N: NetworkSpec, E: ExecutionProivder<N>> {
    accounts: HashMap<Address, Account>,
    block_hash: HashMap<u64, B256>,
    block: BlockId,
    access: Option<StateAccess>,
    execution: Arc<E>,
    phantom: PhantomData<N>,
}

impl<N: NetworkSpec, E: ExecutionProivder<N>> EvmState<N, E> {
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

impl<N: NetworkSpec, E: ExecutionProivder<N>> Database for ProofDB<N, E> {
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
