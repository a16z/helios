use std::{collections::HashMap, str::FromStr, sync::Arc};

use bytes::Bytes;
use common::types::BlockTag;
use ethers::types::transaction::eip2930::AccessListItem;
use eyre::{Report, Result};
use futures::future::join_all;
use log::trace;
use revm::{
    primitives::{
        AccountInfo, Bytecode, Env, ExecutionResult, ResultAndState, TransactTo, B160, B256, U256,
    },
    Database, EVM,
};

use crate::{
    constants::PARALLEL_QUERY_BATCH_SIZE, errors::EvmError, rpc::ExecutionRpc, types::CallOpts,
};

use super::ExecutionClient;

pub struct Evm<R: ExecutionRpc> {
    evm: EVM<ProofDB<R>>,
    chain_id: u64,
    tag: BlockTag,
}

impl<R: ExecutionRpc> Evm<R> {
    pub fn new(execution: Arc<ExecutionClient<R>>, chain_id: u64, tag: BlockTag) -> Self {
        let mut evm: EVM<ProofDB<R>> = EVM::new();
        let db = ProofDB::new(tag, execution);
        evm.database(db);

        Evm { evm, chain_id, tag }
    }

    pub async fn call(&mut self, opts: &CallOpts) -> Result<Vec<u8>, EvmError> {
        let tx = self.call_inner(opts).await?;

        match tx.result {
            ExecutionResult::Success { output, .. } => Ok(output.into_data().to_vec()),
            ExecutionResult::Revert { output, .. } => Err(EvmError::Revert(Some(output))),
            ExecutionResult::Halt { .. } => Err(EvmError::Revert(None)),
        }
    }

    pub async fn estimate_gas(&mut self, opts: &CallOpts) -> Result<u64, EvmError> {
        let tx = self.call_inner(opts).await?;

        match tx.result {
            ExecutionResult::Success { gas_used, .. } => Ok(gas_used),
            ExecutionResult::Revert { gas_used, .. } => Ok(gas_used),
            ExecutionResult::Halt { gas_used, .. } => Ok(gas_used),
        }
    }

    async fn call_inner(&mut self, opts: &CallOpts) -> Result<ResultAndState, EvmError> {
        let env = self.get_env(opts, self.tag).await;
        self.evm
            .db
            .as_mut()
            .unwrap()
            .state
            .prefetch_state(&opts)
            .await
            .map_err(|err| EvmError::Generic(err.to_string()))?;

        let tx_res = loop {
            self.evm.env = env.clone();
            let res = self.evm.transact();
            let mut db = self.evm.db.take().unwrap();

            if res.is_err() && db.state.needs_update() {
                println!("updating state");
                db.state.update_state().await.unwrap();
                self.evm = EVM::<ProofDB<R>>::new();
                self.evm.database(db);
            } else {
                println!("breaking");
                break res;
            }
        };

        tx_res.map_err(|_| EvmError::Generic("evm error".to_string()))
    }

    async fn get_env(&self, opts: &CallOpts, tag: BlockTag) -> Env {
        let mut env = Env::default();

        env.tx.transact_to = TransactTo::Call(opts.to.unwrap_or_default().into());
        env.tx.caller = opts
            .from
            .map(|caller| B160::from(caller))
            .unwrap_or_default();
        env.tx.value = opts
            .value
            .map(|value| B256::from(value).into())
            .unwrap_or_default();
        env.tx.data = Bytes::from(opts.data.clone().unwrap_or_default().to_vec());
        env.tx.gas_limit = opts.gas.map(|v| v.as_u64()).unwrap_or(u64::MAX);
        env.tx.gas_price = opts
            .gas_price
            .map(|g| B256::from(g).into())
            .unwrap_or_default();

        let block = self
            .evm
            .db
            .as_ref()
            .unwrap()
            .execution
            .get_block(tag, false)
            .await
            .unwrap();

        env.block.number = U256::from(block.number.as_u64());
        env.block.coinbase = block.miner.into();
        env.block.timestamp = U256::from(block.timestamp.as_u64());
        env.block.difficulty = block.difficulty.into();

        env.cfg.chain_id = U256::from(self.chain_id);

        env
    }
}

struct ProofDB<R: ExecutionRpc> {
    execution: Arc<ExecutionClient<R>>,
    state: EvmState<R>,
}

impl<'a, R: ExecutionRpc> ProofDB<R> {
    pub fn new(tag: BlockTag, execution: Arc<ExecutionClient<R>>) -> Self {
        let state = EvmState::new(execution.clone(), tag);
        ProofDB { execution, state }
    }
}

enum StateAccess {
    Basic(B160),
    BlockHash(u64),
    Storage(B160, U256),
}

struct EvmState<R: ExecutionRpc> {
    basic: HashMap<B160, AccountInfo>,
    block_hash: HashMap<u64, B256>,
    storage: HashMap<B160, HashMap<U256, U256>>,
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
                        .get_account(&(*address).into(), None, self.block)
                        .await?;
                    let bytecode = Bytecode::new_raw(account.code.into());
                    let account = AccountInfo::new(account.balance.into(), account.nonce, bytecode);
                    self.basic.insert(*address, account);
                }
                StateAccess::Storage(address, slot) => {
                    let slot_ethers = ethers::types::H256::from_slice(&slot.to_be_bytes::<32>());
                    let slots = [slot_ethers];
                    let account = self
                        .execution
                        .get_account(&(*address).into(), Some(&slots), self.block)
                        .await?;

                    let storage = self.storage.entry(*address).or_default();
                    let value = *account.slots.get(&slot_ethers).unwrap();
                    storage.insert(*slot, value.into());
                }
                StateAccess::BlockHash(number) => {
                    let block = self
                        .execution
                        .get_block(BlockTag::Number(*number), false)
                        .await?;
                    self.block_hash.insert(*number, block.hash.into());
                }
            }
        }

        Ok(())
    }

    pub fn needs_update(&self) -> bool {
        self.access.is_some()
    }

    pub fn get_basic(&mut self, address: B160) -> Result<AccountInfo> {
        if let Some(account) = self.basic.get(&address) {
            Ok(account.clone())
        } else {
            self.access = Some(StateAccess::Basic(address));
            eyre::bail!("state missing");
        }
    }

    pub fn get_storage(&mut self, address: B160, slot: U256) -> Result<U256> {
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

    pub async fn prefetch_state(&mut self, opts: &CallOpts) -> Result<()> {
        let mut list = self
            .execution
            .rpc
            .create_access_list(&opts, self.block)
            .await
            .map_err(EvmError::RpcError)?
            .0;

        let from_access_entry = AccessListItem {
            address: opts.from.unwrap_or_default(),
            storage_keys: Vec::default(),
        };

        let to_access_entry = AccessListItem {
            address: opts.to.unwrap_or_default(),
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
            let info = AccountInfo::new(
                account.balance.into(),
                account.nonce.into(),
                Bytecode::new_raw(account.code.into()),
            );

            self.basic.insert(address.into(), info);

            for (slot, value) in account.slots {
                self.storage
                    .entry(address.into())
                    .or_default()
                    .insert(B256::from(slot).into(), value.into());
            }
        }

        Ok(())
    }
}

impl<R: ExecutionRpc> Database for ProofDB<R> {
    type Error = Report;

    fn basic(&mut self, address: B160) -> Result<Option<AccountInfo>, Report> {
        if is_precompile(&address.into()) {
            return Ok(Some(AccountInfo::default()));
        }

        trace!(
            "fetch basic evm state for address=0x{}",
            hex::encode(address.as_bytes())
        );

        Ok(Some(self.state.get_basic(address.into())?))
    }

    fn block_hash(&mut self, number: U256) -> Result<B256, Report> {
        trace!("fetch block hash for block={:?}", number);
        let number_ethers: ethers::types::U256 = number.into();
        self.state.get_block_hash(number_ethers.as_u64())
    }

    fn storage(&mut self, address: B160, slot: U256) -> Result<U256, Report> {
        trace!("fetch evm state for address={:?}, slot={}", address, slot);
        self.state.get_storage(address, slot)
    }

    fn code_by_hash(&mut self, _code_hash: B256) -> Result<Bytecode, Report> {
        Err(eyre::eyre!("should never be called"))
    }
}

fn is_precompile(address: &B160) -> bool {
    address.le(&B160::from_str("0x0000000000000000000000000000000000000009").unwrap())
        && address.gt(&B160::zero())
}

#[cfg(test)]
mod tests {
    use common::utils::hex_str_to_bytes;
    use consensus::types::{primitives::ByteVector, ExecutionPayloadBellatrix};

    use crate::rpc::mock_rpc::MockRpc;

    use super::*;

    fn get_client() -> ExecutionClient<MockRpc> {
        ExecutionClient::new("testdata/").unwrap()
    }

    #[tokio::test]
    async fn test_proof_db() {
        // Construct proofdb params
        let execution = get_client();
        let address = Address::from_str("14f9D4aF749609c1438528C0Cce1cC3f6D411c47").unwrap();
        let payload = ExecutionPayload::Bellatrix(ExecutionPayloadBellatrix {
            state_root: ByteVector::try_from(
                hex_str_to_bytes(
                    "0xaa02f5db2ee75e3da400d10f3c30e894b6016ce8a2501680380a907b6674ce0d",
                )
                .unwrap(),
            )
            .unwrap(),
            ..ExecutionPayloadBellatrix::default()
        });

        let mut payloads = BTreeMap::new();
        payloads.insert(7530933, payload.clone());

        // Construct the proof database with the given client and payloads
        let mut proof_db = ProofDB::new(Arc::new(execution), &payload, &payloads);

        // Set the proof db accounts
        let slot = U256::from(1337);
        let mut accounts = HashMap::new();
        let account = Account {
            balance: U256::from(100),
            code: hex_str_to_bytes("0x").unwrap(),
            ..Default::default()
        };
        accounts.insert(address, account);
        proof_db.set_accounts(accounts);

        // Get the account from the proof database
        let storage_proof = proof_db.storage(address, slot);

        // Check that the storage proof correctly returns a slot not found error
        let expected_err: eyre::Report = SlotNotFoundError::new(H256::from_uint(&slot)).into();
        assert_eq!(
            expected_err.to_string(),
            storage_proof.unwrap_err().to_string()
        );
    }
}
