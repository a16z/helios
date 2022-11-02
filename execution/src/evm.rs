use std::{collections::HashMap, fmt::Display, str::FromStr, thread};

use bytes::Bytes;
use ethers::{
    abi::ethereum_types::BigEndianHash,
    prelude::{Address, H160, H256, U256},
    types::transaction::eip2930::AccessListItem,
};
use eyre::Result;
use futures::future::join_all;
use log::trace;
use revm::{AccountInfo, Bytecode, Database, Env, TransactOut, TransactTo, EVM};
use tokio::runtime::Runtime;

use consensus::types::ExecutionPayload;

use crate::{
    rpc::ExecutionRpc,
    types::{Account, CallOpts},
};

use super::ExecutionClient;

pub struct Evm<R: ExecutionRpc> {
    evm: EVM<ProofDB<R>>,
    chain_id: u64,
}

impl<R: ExecutionRpc> Evm<R> {
    pub fn new(execution: ExecutionClient<R>, payload: ExecutionPayload, chain_id: u64) -> Self {
        let mut evm: EVM<ProofDB<R>> = EVM::new();
        let db = ProofDB::new(execution, payload);
        evm.database(db);

        Evm { evm, chain_id }
    }

    pub fn call(&mut self, opts: &CallOpts) -> Result<Vec<u8>> {
        let account_map = self.batch_fetch_accounts(opts)?;
        self.evm.db.as_mut().unwrap().set_accounts(account_map);

        self.evm.env = self.get_env(opts);
        let output = self.evm.transact().1;

        if let Some(err) = &self.evm.db.as_ref().unwrap().error {
            return Err(eyre::eyre!(err.clone()));
        }

        match output {
            TransactOut::None => Err(eyre::eyre!("Invalid Call")),
            TransactOut::Create(..) => Err(eyre::eyre!("Invalid Call")),
            TransactOut::Call(bytes) => Ok(bytes.to_vec()),
        }
    }

    pub fn estimate_gas(&mut self, opts: &CallOpts) -> Result<u64> {
        let account_map = self.batch_fetch_accounts(opts)?;
        self.evm.db.as_mut().unwrap().set_accounts(account_map);

        self.evm.env = self.get_env(opts);
        let gas = self.evm.transact().2;

        if let Some(err) = &self.evm.db.as_ref().unwrap().error {
            return Err(eyre::eyre!(err.clone()));
        }

        // overestimate to avoid out of gas reverts
        let gas_scaled = (1.10 * gas as f64) as u64;
        Ok(gas_scaled)
    }

    fn batch_fetch_accounts(&self, opts: &CallOpts) -> Result<HashMap<Address, Account>> {
        let db = self.evm.db.as_ref().unwrap();
        let rpc = db.execution.rpc.clone();
        let payload = db.payload.clone();
        let execution = db.execution.clone();
        let block = db.payload.block_number;

        let opts_moved = CallOpts {
            from: opts.from,
            to: opts.to,
            value: opts.value,
            data: opts.data.clone(),
            gas: opts.gas,
            gas_price: opts.gas_price,
        };

        let block_moved = block.clone();
        let handle = thread::spawn(move || {
            let list_fut = rpc.create_access_list(&opts_moved, block_moved);
            let runtime = Runtime::new()?;
            let mut list = runtime.block_on(list_fut)?.0;

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

            let account_futs = list.iter().map(|account| {
                let addr_fut = futures::future::ready(account.address);
                let account_fut = execution.get_account(
                    &account.address,
                    Some(account.storage_keys.as_slice()),
                    &payload,
                );
                async move { (addr_fut.await, account_fut.await) }
            });

            let accounts = runtime.block_on(join_all(account_futs));

            Ok::<_, eyre::Error>(accounts)
        });

        let accounts = handle.join().unwrap()?;
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
        let payload = &self.evm.db.as_ref().unwrap().payload;

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

struct ProofDB<R: ExecutionRpc> {
    execution: ExecutionClient<R>,
    payload: ExecutionPayload,
    accounts: HashMap<Address, Account>,
    error: Option<String>,
}

impl<R: ExecutionRpc> ProofDB<R> {
    pub fn new(execution: ExecutionClient<R>, payload: ExecutionPayload) -> Self {
        ProofDB {
            execution,
            payload,
            accounts: HashMap::new(),
            error: None,
        }
    }

    pub fn safe_unwrap<T: Default, E: Display>(&mut self, res: Result<T, E>) -> T {
        match res {
            Ok(value) => value,
            Err(err) => {
                self.error = Some(err.to_string());
                T::default()
            }
        }
    }

    pub fn set_accounts(&mut self, accounts: HashMap<Address, Account>) {
        self.accounts = accounts;
    }

    fn get_account(&mut self, address: Address, slots: &[H256]) -> Account {
        let execution = self.execution.clone();
        let addr = address.clone();
        let payload = self.payload.clone();
        let slots = slots.to_owned();

        let handle = thread::spawn(move || {
            let account_fut = execution.get_account(&addr, Some(&slots), &payload);
            let runtime = Runtime::new()?;
            runtime.block_on(account_fut)
        });

        self.safe_unwrap(handle.join().unwrap())
    }
}

impl<R: ExecutionRpc> Database for ProofDB<R> {
    fn basic(&mut self, address: H160) -> AccountInfo {
        if is_precompile(&address) {
            return AccountInfo::default();
        }

        trace!(
            "fetch basic evm state for addess=0x{}",
            hex::encode(address.as_bytes())
        );

        let account = match self.accounts.get(&address) {
            Some(account) => account.clone(),
            None => self.get_account(address, &[]),
        };

        let bytecode = Bytecode::new_raw(Bytes::from(account.code.clone()));
        AccountInfo::new(account.balance, account.nonce, bytecode)
    }

    fn block_hash(&mut self, _number: U256) -> H256 {
        H256::default()
    }

    fn storage(&mut self, address: H160, slot: U256) -> U256 {
        trace!(
            "fetch evm state for address=0x{}, slot={}",
            hex::encode(address.as_bytes()),
            slot
        );

        let slot = H256::from_uint(&slot);

        match self.accounts.get(&address) {
            Some(account) => match account.slots.get(&slot) {
                Some(slot) => slot.clone(),
                None => self
                    .get_account(address, &[slot])
                    .slots
                    .get(&slot)
                    .unwrap()
                    .clone(),
            },
            None => self
                .get_account(address, &[slot])
                .slots
                .get(&slot)
                .unwrap()
                .clone(),
        }
    }

    fn code_by_hash(&mut self, _code_hash: H256) -> Bytecode {
        panic!("should never be called");
    }
}

fn is_precompile(address: &Address) -> bool {
    address.le(&Address::from_str("0x0000000000000000000000000000000000000009").unwrap())
        && address.gt(&Address::zero())
}
