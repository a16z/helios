use std::str::FromStr;

use bytes::Bytes;
use eyre::Result;
use ethers::prelude::{U256, H256, H160, Address};
use revm::{Database, Bytecode, AccountInfo, EVM, Env, TransactTo, TransactOut};
use futures::executor::block_on;

use crate::consensus::types::ExecutionPayload;
use super::ExecutionClient;

pub struct Evm {
    evm: EVM<ProofDB>
}

impl Evm {
    pub fn new(execution: ExecutionClient, payload: ExecutionPayload) -> Self {
        let mut evm: EVM<ProofDB> = EVM::new();
        let db = ProofDB::new(execution, payload);
        evm.database(db);

        Evm { evm }
    }

    pub fn call(&mut self, to: &Address, calldata: &Vec<u8>, value: U256) -> Result<Vec<u8>> {

        let mut env = Env::default();
        let mut tx = revm::TxEnv::default();
        tx.transact_to = TransactTo::Call(*to);
        tx.data = Bytes::from(calldata.clone());
        tx.value = value;
        env.tx = tx;

        self.evm.env = env;

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
            },
        }
    }
}

impl Database for ProofDB {
    fn basic(&mut self, address: H160) -> AccountInfo {

        if is_precompile(&address) {
            return AccountInfo::default()
        }

        let account_future = self.execution.get_account(&address, None, &self.payload);
        let account = self.safe_unwrap(block_on(account_future));

        let bytecode_future = self.execution.get_code(&address, &self.payload);
        let bytecode = self.safe_unwrap(block_on(bytecode_future));
        let bytecode = Bytecode::new_raw(Bytes::from(bytecode));

        AccountInfo::new(account.balance, account.nonce.as_u64(), bytecode)
    }

    fn block_hash(&mut self, _number: U256) -> H256 {
        H256::default()
    }

    fn storage(&mut self, address: H160, slot: U256) -> U256 {
        let slots = [slot];
        let account_future = self.execution.get_account(&address, Some(&slots), &self.payload);
        let account = self.safe_unwrap(block_on(account_future));

        *account.slots.get(&slot).unwrap()
    }

    fn code_by_hash(&mut self, _code_hash: H256) -> Bytecode {
        panic!("should never be called");
    }
}

fn is_precompile(address: &Address) -> bool {
    address.le(&Address::from_str("0x0000000000000000000000000000000000000009").unwrap())
}
