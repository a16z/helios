use std::{borrow::BorrowMut, collections::HashMap, sync::Arc};

use alloy::network::{primitives::HeaderResponse, BlockResponse};
use eyre::{Report, Result};
use revm::{
    primitives::{
        address, AccountInfo, Address, Bytecode, Bytes, CfgEnv, Env, ExecutionResult,
        ResultAndState, B256, U256,
    },
    Database, Evm as Revm,
};
use tracing::trace;

use helios_common::{fork_schedule::ForkSchedule, network_spec::NetworkSpec, types::BlockTag};
use helios_verifiable_api_client::VerifiableApi;

use crate::execution::{
    errors::{EvmError, ExecutionError},
    rpc::ExecutionRpc,
    ExecutionClient,
};

pub struct Evm<N: NetworkSpec, R: ExecutionRpc<N>, A: VerifiableApi<N>> {
    execution: Arc<ExecutionClient<N, R, A>>,
    chain_id: u64,
    tag: BlockTag,
    fork_schedule: ForkSchedule,
}

impl<N: NetworkSpec, R: ExecutionRpc<N>, A: VerifiableApi<N>> Evm<N, R, A> {
    pub fn new(
        execution: Arc<ExecutionClient<N, R, A>>,
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
        let tx = self.call_inner(tx).await?;

        match tx.result {
            ExecutionResult::Success { output, .. } => Ok(output.into_data()),
            ExecutionResult::Revert { output, .. } => {
                Err(EvmError::Revert(Some(output.to_vec().into())))
            }
            ExecutionResult::Halt { .. } => Err(EvmError::Revert(None)),
        }
    }

    pub async fn estimate_gas(&mut self, tx: &N::TransactionRequest) -> Result<u64, EvmError> {
        let tx = self.call_inner(tx).await?;

        match tx.result {
            ExecutionResult::Success { gas_used, .. } => Ok(gas_used),
            ExecutionResult::Revert { gas_used, .. } => Ok(gas_used),
            ExecutionResult::Halt { gas_used, .. } => Ok(gas_used),
        }
    }

    async fn call_inner(&mut self, tx: &N::TransactionRequest) -> Result<ResultAndState, EvmError> {
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

    async fn get_env(&self, tx: &N::TransactionRequest, tag: BlockTag) -> Env {
        let block = self
            .execution
            .get_block(tag, false)
            .await
            .ok_or(ExecutionError::BlockNotFound(tag))
            .unwrap();

        let mut cfg = CfgEnv::default();
        cfg.chain_id = self.chain_id;
        cfg.disable_block_gas_limit = true;
        cfg.disable_eip3607 = true;
        cfg.disable_base_fee = true;

        Env {
            tx: N::tx_env(tx),
            block: N::block_env(&block, &self.fork_schedule),
            cfg,
        }
    }
}

struct ProofDB<N: NetworkSpec, R: ExecutionRpc<N>, A: VerifiableApi<N>> {
    state: EvmState<N, R, A>,
}

impl<N: NetworkSpec, R: ExecutionRpc<N>, A: VerifiableApi<N>> ProofDB<N, R, A> {
    pub fn new(tag: BlockTag, execution: Arc<ExecutionClient<N, R, A>>) -> Self {
        let state = EvmState::new(execution.clone(), tag);
        ProofDB { state }
    }
}

enum StateAccess {
    Basic(Address),
    BlockHash(u64),
    Storage(Address, U256),
}

struct EvmState<N: NetworkSpec, R: ExecutionRpc<N>, A: VerifiableApi<N>> {
    basic: HashMap<Address, AccountInfo>,
    block_hash: HashMap<u64, B256>,
    storage: HashMap<Address, HashMap<U256, U256>>,
    block: BlockTag,
    access: Option<StateAccess>,
    execution: Arc<ExecutionClient<N, R, A>>,
}

impl<N: NetworkSpec, R: ExecutionRpc<N>, A: VerifiableApi<N>> EvmState<N, R, A> {
    pub fn new(execution: Arc<ExecutionClient<N, R, A>>, block: BlockTag) -> Self {
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
                        .get_account(*address, None, self.block, true)
                        .await?;

                    self.basic.insert(
                        *address,
                        AccountInfo::new(
                            account.balance,
                            account.nonce,
                            account.code_hash,
                            Bytecode::new_raw(account.code.unwrap().into()),
                        ),
                    );
                }
                StateAccess::Storage(address, slot) => {
                    let slot_bytes = B256::from(*slot);
                    let account = self
                        .execution
                        .get_account(*address, Some(&[slot_bytes]), self.block, false)
                        .await?;

                    let storage = self.storage.entry(*address).or_default();
                    let value = *account.slots.get(&slot_bytes).unwrap();
                    storage.insert(*slot, value);
                }
                StateAccess::BlockHash(number) => {
                    let tag = BlockTag::Number(*number);
                    let block = self
                        .execution
                        .get_block(tag, false)
                        .await
                        .ok_or(ExecutionError::BlockNotFound(tag))?;

                    self.block_hash.insert(*number, block.header().hash());
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

    pub async fn prefetch_state(&mut self, tx: &N::TransactionRequest) -> Result<()> {
        let block_id = Some(self.block.into());
        let account_map = self
            .execution
            .create_access_list(tx, block_id)
            .await
            .map_err(EvmError::RpcError)?;

        for (address, account) in account_map {
            self.basic.insert(
                address,
                AccountInfo::new(
                    account.balance,
                    account.nonce,
                    account.code_hash,
                    Bytecode::new_raw(account.code.unwrap().into()),
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

impl<N: NetworkSpec, R: ExecutionRpc<N>, A: VerifiableApi<N>> Database for ProofDB<N, R, A> {
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
