use std::{
    cmp,
    collections::{BTreeMap, HashMap},
    str::FromStr,
    sync::Arc,
    thread,
};

use bytes::Bytes;
use common::{errors::BlockNotFoundError, types::BlockTag};
use ethers::{
    abi::ethereum_types::BigEndianHash,
    prelude::{Address, H160, H256, U256},
    types::transaction::eip2930::AccessListItem,
};
use eyre::{Report, Result};
use futures::future::join_all;
use log::trace;
use revm::{AccountInfo, Bytecode, Database, Env, TransactOut, TransactTo, EVM};
use tokio::runtime::Runtime;

use consensus::types::ExecutionPayload;

use crate::{
    errors::EvmError,
    rpc::ExecutionRpc,
    types::{Account, CallOpts},
};

use super::ExecutionClient;

pub struct Evm<'a, R: ExecutionRpc> {
    evm: EVM<ProofDB<'a, R>>,
    chain_id: u64,
}

impl<'a, R: ExecutionRpc> Evm<'a, R> {
    pub fn new(
        execution: Arc<ExecutionClient<R>>,
        current_payload: &'a ExecutionPayload,
        payloads: &'a BTreeMap<u64, ExecutionPayload>,
        chain_id: u64,
    ) -> Self {
        let mut evm: EVM<ProofDB<R>> = EVM::new();
        let db = ProofDB::new(execution, current_payload, payloads);
        evm.database(db);

        Evm { evm, chain_id }
    }

    pub async fn call(&mut self, opts: &CallOpts) -> Result<Vec<u8>, EvmError> {
        let account_map = self.batch_fetch_accounts(opts).await?;
        self.evm.db.as_mut().unwrap().set_accounts(account_map);

        self.evm.env = self.get_env(opts);
        let tx = self.evm.transact().0;

        match tx.exit_reason {
            revm::Return::Revert => match tx.out {
                TransactOut::Call(bytes) => Err(EvmError::Revert(Some(bytes))),
                _ => Err(EvmError::Revert(None)),
            },
            revm::Return::Return => {
                if let Some(err) = &self.evm.db.as_ref().unwrap().error {
                    return Err(EvmError::Generic(err.clone()));
                }

                match tx.out {
                    TransactOut::None => Err(EvmError::Generic("Invalid Call".to_string())),
                    TransactOut::Create(..) => Err(EvmError::Generic("Invalid Call".to_string())),
                    TransactOut::Call(bytes) => Ok(bytes.to_vec()),
                }
            }
            _ => Err(EvmError::Revm(tx.exit_reason)),
        }
    }

    pub async fn estimate_gas(&mut self, opts: &CallOpts) -> Result<u64, EvmError> {
        let account_map = self.batch_fetch_accounts(opts).await?;
        self.evm.db.as_mut().unwrap().set_accounts(account_map);

        self.evm.env = self.get_env(opts);
        let tx = self.evm.transact().0;
        let gas = tx.gas_used;

        match tx.exit_reason {
            revm::Return::Revert => match tx.out {
                TransactOut::Call(bytes) => Err(EvmError::Revert(Some(bytes))),
                _ => Err(EvmError::Revert(None)),
            },
            revm::Return::Return => {
                if let Some(err) = &self.evm.db.as_ref().unwrap().error {
                    return Err(EvmError::Generic(err.clone()));
                }

                // overestimate to avoid out of gas reverts
                let gas_scaled = (1.10 * gas as f64) as u64;
                Ok(gas_scaled)
            }
            _ => Err(EvmError::Revm(tx.exit_reason)),
        }
    }

    async fn batch_fetch_accounts(
        &self,
        opts: &CallOpts,
    ) -> Result<HashMap<Address, Account>, EvmError> {
        let db = self.evm.db.as_ref().unwrap();
        let rpc = db.execution.rpc.clone();
        let payload = db.current_payload.clone();
        let execution = db.execution.clone();
        let block = db.current_payload.block_number;

        let opts_moved = CallOpts {
            from: opts.from,
            to: opts.to,
            value: opts.value,
            data: opts.data.clone(),
            gas: opts.gas,
            gas_price: opts.gas_price,
        };

        let block_moved = block.clone();
        let mut list = rpc
            .create_access_list(&opts_moved, block_moved)
            .await
            .map_err(EvmError::RpcError)?
            .0;

        let from_access_entry = AccessListItem {
            address: opts_moved.from.unwrap_or_default(),
            storage_keys: Vec::default(),
        };

        let to_access_entry = AccessListItem {
            address: opts_moved.to,
            storage_keys: Vec::default(),
        };

        let producer_account = AccessListItem {
            address: Address::from_slice(&payload.fee_recipient),
            storage_keys: Vec::default(),
        };

        list.push(from_access_entry);
        list.push(to_access_entry);
        list.push(producer_account);

        let mut accounts = Vec::new();
        let batch_size = 20;
        for i in (0..list.len()).step_by(batch_size) {
            let end = cmp::min(i + batch_size, list.len());
            let chunk = &list[i..end];

            let account_chunk_futs = chunk.iter().map(|account| {
                let addr_fut = futures::future::ready(account.address);
                let account_fut = execution.get_account(
                    &account.address,
                    Some(account.storage_keys.as_slice()),
                    &payload,
                );
                async move { (addr_fut.await, account_fut.await) }
            });

            let mut account_chunk = join_all(account_chunk_futs).await;
            accounts.append(&mut account_chunk);
        }

        let mut account_map = HashMap::new();
        accounts.iter().for_each(|account| {
            let addr = account.0;
            if let Ok(account) = &account.1 {
                account_map.insert(addr, account.clone());
            }
        });

        Ok(account_map)
    }

    fn get_env(&self, opts: &CallOpts) -> Env {
        let mut env = Env::default();
        let payload = &self.evm.db.as_ref().unwrap().current_payload;

        env.tx.transact_to = TransactTo::Call(opts.to);
        env.tx.caller = opts.from.unwrap_or(Address::zero());
        env.tx.value = opts.value.unwrap_or(U256::from(0));
        env.tx.data = Bytes::from(opts.data.clone().unwrap_or(vec![]));
        env.tx.gas_limit = opts.gas.map(|v| v.as_u64()).unwrap_or(u64::MAX);
        env.tx.gas_price = opts.gas_price.unwrap_or(U256::zero());

        env.block.number = U256::from(payload.block_number);
        env.block.coinbase = Address::from_slice(&payload.fee_recipient);
        env.block.timestamp = U256::from(payload.timestamp);
        env.block.difficulty = U256::from_little_endian(&payload.prev_randao);

        env.cfg.chain_id = self.chain_id.into();

        env
    }
}

struct ProofDB<'a, R: ExecutionRpc> {
    execution: Arc<ExecutionClient<R>>,
    current_payload: &'a ExecutionPayload,
    payloads: &'a BTreeMap<u64, ExecutionPayload>,
    accounts: HashMap<Address, Account>,
    error: Option<String>,
}

impl<'a, R: ExecutionRpc> ProofDB<'a, R> {
    pub fn new(
        execution: Arc<ExecutionClient<R>>,
        current_payload: &'a ExecutionPayload,
        payloads: &'a BTreeMap<u64, ExecutionPayload>,
    ) -> Self {
        ProofDB {
            execution,
            current_payload,
            payloads,
            accounts: HashMap::new(),
            error: None,
        }
    }

    pub fn set_accounts(&mut self, accounts: HashMap<Address, Account>) {
        self.accounts = accounts;
    }

    fn get_account(&mut self, address: Address, slots: &[H256]) -> Result<Account> {
        let execution = self.execution.clone();
        let addr = address.clone();
        let payload = self.current_payload.clone();
        let slots = slots.to_owned();

        let handle = thread::spawn(move || {
            let account_fut = execution.get_account(&addr, Some(&slots), &payload);
            let runtime = Runtime::new()?;
            runtime.block_on(account_fut)
        });

        handle.join().unwrap()
    }
}

impl<'a, R: ExecutionRpc> Database for ProofDB<'a, R> {
    type Error = Report;

    fn basic(&mut self, address: H160) -> Result<Option<AccountInfo>, Report> {
        if is_precompile(&address) {
            return Ok(Some(AccountInfo::default()));
        }

        trace!(
            "fetch basic evm state for addess=0x{}",
            hex::encode(address.as_bytes())
        );

        let account = match self.accounts.get(&address) {
            Some(account) => account.clone(),
            None => self.get_account(address, &[])?,
        };

        let bytecode = Bytecode::new_raw(Bytes::from(account.code.clone()));
        Ok(Some(AccountInfo::new(
            account.balance,
            account.nonce,
            bytecode,
        )))
    }

    fn block_hash(&mut self, number: U256) -> Result<H256, Report> {
        let number = number.as_u64();
        let payload = self
            .payloads
            .get(&number)
            .ok_or(BlockNotFoundError::new(BlockTag::Number(number)))?;
        Ok(H256::from_slice(&payload.block_hash))
    }

    fn storage(&mut self, address: H160, slot: U256) -> Result<U256, Report> {
        trace!(
            "fetch evm state for address=0x{}, slot={}",
            hex::encode(address.as_bytes()),
            slot
        );

        let slot = H256::from_uint(&slot);

        Ok(match self.accounts.get(&address) {
            Some(account) => match account.slots.get(&slot) {
                Some(slot) => slot.clone(),
                None => self
                    .get_account(address, &[slot])?
                    .slots
                    .get(&slot)
                    .unwrap()
                    .clone(),
            },
            None => self
                .get_account(address, &[slot])?
                .slots
                .get(&slot)
                .unwrap()
                .clone(),
        })
    }

    fn code_by_hash(&mut self, _code_hash: H256) -> Result<Bytecode, Report> {
        Err(eyre::eyre!("should never be called"))
    }
}

fn is_precompile(address: &Address) -> bool {
    address.le(&Address::from_str("0x0000000000000000000000000000000000000009").unwrap())
        && address.gt(&Address::zero())
}
