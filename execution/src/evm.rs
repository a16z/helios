use std::{str::FromStr, thread};

use bytes::Bytes;
use ethers::prelude::{Address, H160, H256, U256};
use eyre::Result;
use revm::{AccountInfo, Bytecode, Database, Env, TransactOut, TransactTo, EVM};
use tokio::runtime::Runtime;

use consensus::types::ExecutionPayload;

use crate::types::CallOpts;

use super::ExecutionClient;

pub struct Evm {
    evm: EVM<ProofDB>,
    chain_id: u64,
}

impl Evm {
    pub fn new(execution: ExecutionClient, payload: ExecutionPayload, chain_id: u64) -> Self {
        let mut evm: EVM<ProofDB> = EVM::new();
        let db = ProofDB::new(execution, payload);
        evm.database(db);

        Evm { evm, chain_id }
    }

    pub fn call(&mut self, opts: &CallOpts) -> Result<Vec<u8>> {
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
        self.evm.env = self.get_env(opts);
        let gas = self.evm.transact().2;

        if let Some(err) = &self.evm.db.as_ref().unwrap().error {
            return Err(eyre::eyre!(err.clone()));
        }

        let gas_scaled = (1.10 * gas as f64) as u64;
        Ok(gas_scaled)
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

struct ProofDB {
    execution: ExecutionClient,
    payload: ExecutionPayload,
    error: Option<String>,
}

impl ProofDB {
    pub fn new(execution: ExecutionClient, payload: ExecutionPayload) -> Self {
        ProofDB {
            execution,
            payload,
            error: None,
        }
    }

    pub fn safe_unwrap<T: Default>(&mut self, res: Result<T>) -> T {
        match res {
            Ok(value) => value,
            Err(err) => {
                self.error = Some(err.to_string());
                T::default()
            }
        }
    }
}

impl Database for ProofDB {
    fn basic(&mut self, address: H160) -> AccountInfo {
        if is_precompile(&address) {
            return AccountInfo::default();
        }

        let execution = self.execution.clone();
        let addr = address.clone();
        let payload = self.payload.clone();

        let handle = thread::spawn(move || {
            let account_fut = execution.get_account(&addr, None, &payload);
            let runtime = Runtime::new()?;
            runtime.block_on(account_fut)
        });

        let account = self.safe_unwrap(handle.join().unwrap());

        let execution = self.execution.clone();
        let addr = address.clone();
        let payload = self.payload.clone();

        let handle = thread::spawn(move || {
            let code_fut = execution.get_code(&addr, &payload);
            let runtime = Runtime::new()?;
            runtime.block_on(code_fut)
        });

        let bytecode = self.safe_unwrap(handle.join().unwrap());
        let bytecode = Bytecode::new_raw(Bytes::from(bytecode));

        AccountInfo::new(account.balance, account.nonce.as_u64(), bytecode)
    }

    fn block_hash(&mut self, _number: U256) -> H256 {
        H256::default()
    }

    fn storage(&mut self, address: H160, slot: U256) -> U256 {
        let execution = self.execution.clone();
        let addr = address.clone();
        let slots = [slot];
        let payload = self.payload.clone();

        let handle = thread::spawn(move || {
            let account_fut = execution.get_account(&addr, Some(&slots), &payload);
            let runtime = Runtime::new()?;
            runtime.block_on(account_fut)
        });

        let account = self.safe_unwrap(handle.join().unwrap());
        let value = account.slots.get(&slot);
        match value {
            Some(value) => *value,
            None => {
                self.error = Some("slot not found".to_string());
                U256::default()
            }
        }
    }

    fn code_by_hash(&mut self, _code_hash: H256) -> Bytecode {
        panic!("should never be called");
    }
}

fn is_precompile(address: &Address) -> bool {
    address.le(&Address::from_str("0x0000000000000000000000000000000000000009").unwrap())
}
