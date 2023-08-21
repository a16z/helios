// use std::{
//     collections::{BTreeMap, HashMap},
//     str::FromStr,
//     sync::Arc,
// };
//
// use bytes::Bytes;
// use common::{
//     errors::{BlockNotFoundError, SlotNotFoundError},
//     types::BlockTag,
// };
// use ethers::{
//     abi::ethereum_types::BigEndianHash,
//     types::transaction::eip2930::AccessListItem,
//     types::{Address, H160, H256, U256},
// };
// use eyre::{Report, Result};
// use futures::{executor::block_on, future::join_all};
// use log::trace;
// use revm::{AccountInfo, Bytecode, Database, Env, TransactOut, TransactTo, EVM};
//
// use consensus::types::ExecutionPayload;
// use tokio::task::spawn_blocking;
//
// use crate::{
//     constants::PARALLEL_QUERY_BATCH_SIZE,
//     errors::EvmError,
//     rpc::ExecutionRpc,
//     types::{Account, CallOpts},
// };
//
// use super::ExecutionClient;
//
// pub struct Evm<'a, R: ExecutionRpc> {
//     evm: EVM<ProofDB<'a, R>>,
//     chain_id: u64,
// }
//
// impl<'a, R: ExecutionRpc> Evm<'a, R> {
//     pub fn new(
//         execution: Arc<ExecutionClient<R>>,
//         chain_id: u64,
//     ) -> Self {
//         let mut evm: EVM<ProofDB<R>> = EVM::new();
//         let db = ProofDB::new(execution);
//         evm.database(db);
//
//         Evm { evm, chain_id }
//     }
//
//     pub async fn call(&mut self, opts: &CallOpts) -> Result<Vec<u8>, EvmError> {
//         let account_map = self.batch_fetch_accounts(opts).await?;
//         self.evm.db.as_mut().unwrap().set_accounts(account_map);
//
//         self.evm.env = self.get_env(opts);
//         let tx = self.evm.transact().0;
//
//         match tx.exit_reason {
//             revm::Return::Revert => match tx.out {
//                 TransactOut::Call(bytes) => Err(EvmError::Revert(Some(bytes))),
//                 _ => Err(EvmError::Revert(None)),
//             },
//             revm::Return::Return | revm::Return::Stop => {
//                 if let Some(err) = &self.evm.db.as_ref().unwrap().error {
//                     return Err(EvmError::Generic(err.clone()));
//                 }
//
//                 match tx.out {
//                     TransactOut::None => Err(EvmError::Generic("Invalid Call".to_string())),
//                     TransactOut::Create(..) => Err(EvmError::Generic("Invalid Call".to_string())),
//                     TransactOut::Call(bytes) => Ok(bytes.to_vec()),
//                 }
//             }
//             _ => Err(EvmError::Revm(tx.exit_reason)),
//         }
//     }
//
//     pub async fn estimate_gas(&mut self, opts: &CallOpts) -> Result<u64, EvmError> {
//         let account_map = self.batch_fetch_accounts(opts).await?;
//         self.evm.db.as_mut().unwrap().set_accounts(account_map);
//
//         self.evm.env = self.get_env(opts);
//         let tx = self.evm.transact().0;
//         let gas = tx.gas_used;
//
//         match tx.exit_reason {
//             revm::Return::Revert => match tx.out {
//                 TransactOut::Call(bytes) => Err(EvmError::Revert(Some(bytes))),
//                 _ => Err(EvmError::Revert(None)),
//             },
//             revm::Return::Return | revm::Return::Stop => {
//                 if let Some(err) = &self.evm.db.as_ref().unwrap().error {
//                     return Err(EvmError::Generic(err.clone()));
//                 }
//
//                 // overestimate to avoid out of gas reverts
//                 let gas_scaled = (1.10 * gas as f64) as u64;
//                 Ok(gas_scaled)
//             }
//             _ => Err(EvmError::Revm(tx.exit_reason)),
//         }
//     }
//
//     async fn batch_fetch_accounts(
//         &self,
//         opts: &CallOpts,
//     ) -> Result<HashMap<Address, Account>, EvmError> {
//         let db = self.evm.db.as_ref().unwrap();
//         let rpc = db.execution.rpc.clone();
//         let payload = db.current_payload.clone();
//         let execution = db.execution.clone();
//         let block = *db.current_payload.block_number();
//
//         let opts_moved = CallOpts {
//             from: opts.from,
//             to: opts.to,
//             value: opts.value,
//             data: opts.data.clone(),
//             gas: opts.gas,
//             gas_price: opts.gas_price,
//         };
//
//         let mut list = rpc
//             .create_access_list(&opts_moved, block.into())
//             .await
//             .map_err(EvmError::RpcError)?
//             .0;
//
//         let from_access_entry = AccessListItem {
//             address: opts_moved.from.unwrap_or_default(),
//             storage_keys: Vec::default(),
//         };
//
//         let to_access_entry = AccessListItem {
//             address: opts_moved.to.unwrap_or_default(),
//             storage_keys: Vec::default(),
//         };
//
//         let producer_account = AccessListItem {
//             address: Address::from_slice(payload.fee_recipient()),
//             storage_keys: Vec::default(),
//         };
//
//         list.push(from_access_entry);
//         list.push(to_access_entry);
//         list.push(producer_account);
//
//         let mut account_map = HashMap::new();
//         for chunk in list.chunks(PARALLEL_QUERY_BATCH_SIZE) {
//             let account_chunk_futs = chunk.iter().map(|account| {
//                 let account_fut = execution.get_account(
//                     &account.address,
//                     Some(account.storage_keys.as_slice()),
//                     &payload,
//                 );
//                 async move { (account.address, account_fut.await) }
//             });
//
//             let account_chunk = join_all(account_chunk_futs).await;
//
//             account_chunk
//                 .into_iter()
//                 .filter(|i| i.1.is_ok())
//                 .for_each(|(key, value)| {
//                     account_map.insert(key, value.ok().unwrap());
//                 });
//         }
//
//         Ok(account_map)
//     }
//
//     fn get_env(&self, opts: &CallOpts) -> Env {
//         let mut env = Env::default();
//         // let payload = &self.evm.db.as_ref().unwrap().current_payload;
//
//         env.tx.transact_to = TransactTo::Call(opts.to.unwrap_or_default());
//         env.tx.caller = opts.from.unwrap_or(Address::zero());
//         env.tx.value = opts.value.unwrap_or(U256::from(0));
//         env.tx.data = Bytes::from(opts.data.clone().unwrap_or_default().to_vec());
//         env.tx.gas_limit = opts.gas.map(|v| v.as_u64()).unwrap_or(u64::MAX);
//         env.tx.gas_price = opts.gas_price.unwrap_or(U256::zero());
//
//         env.block.number = U256::from(payload.block_number().as_u64());
//         env.block.coinbase = Address::from_slice(payload.fee_recipient());
//         env.block.timestamp = U256::from(payload.timestamp().as_u64());
//         env.block.difficulty = U256::from_little_endian(payload.prev_randao());
//
//         env.cfg.chain_id = self.chain_id.into();
//
//         env
//     }
// }
//
// struct ProofDB<R: ExecutionRpc> {
//     execution: Arc<ExecutionClient<R>>,
//     accounts: HashMap<Address, Account>,
//     error: Option<String>,
// }
//
// impl<'a, R: ExecutionRpc> ProofDB<R> {
//     pub fn new(
//         execution: Arc<ExecutionClient<R>>,
//     ) -> Self {
//         ProofDB {
//             execution,
//             accounts: HashMap::new(),
//             error: None,
//         }
//     }
//
//     pub fn set_accounts(&mut self, accounts: HashMap<Address, Account>) {
//         self.accounts = accounts;
//     }
//
//     fn get_account(&mut self, address: Address, slots: &[H256]) -> Result<Account> {
//         let execution = self.execution.clone();
//         let payload = self.current_payload.clone();
//         let slots = slots.to_owned();
//
//         let handle = spawn_blocking(move || {
//             block_on(execution.get_account(&address, Some(&slots), &payload))
//         });
//
//         block_on(handle)?
//     }
// }
//
// impl<'a, R: ExecutionRpc> Database for ProofDB<'a, R> {
//     type Error = Report;
//
//     fn basic(&mut self, address: H160) -> Result<Option<AccountInfo>, Report> {
//         if is_precompile(&address) {
//             return Ok(Some(AccountInfo::default()));
//         }
//
//         trace!(
//             "fetch basic evm state for address=0x{}",
//             hex::encode(address.as_bytes())
//         );
//
//         let account = match self.accounts.get(&address) {
//             Some(account) => account.clone(),
//             None => self.get_account(address, &[])?,
//         };
//
//         let bytecode = Bytecode::new_raw(Bytes::from(account.code.clone()));
//         Ok(Some(AccountInfo::new(
//             account.balance,
//             account.nonce,
//             bytecode,
//         )))
//     }
//
//     fn block_hash(&mut self, number: U256) -> Result<H256, Report> {
//         let number = number.as_u64();
//         let payload = self
//             .payloads
//             .get(&number)
//             .ok_or(BlockNotFoundError::new(BlockTag::Number(number)))?;
//         Ok(H256::from_slice(payload.block_hash()))
//     }
//
//     fn storage(&mut self, address: H160, slot: U256) -> Result<U256, Report> {
//         trace!("fetch evm state for address={:?}, slot={}", address, slot);
//
//         let slot = H256::from_uint(&slot);
//
//         Ok(match self.accounts.get(&address) {
//             Some(account) => match account.slots.get(&slot) {
//                 Some(slot) => *slot,
//                 None => *self
//                     .get_account(address, &[slot])?
//                     .slots
//                     .get(&slot)
//                     .ok_or(SlotNotFoundError::new(slot))?,
//             },
//             None => *self
//                 .get_account(address, &[slot])?
//                 .slots
//                 .get(&slot)
//                 .ok_or(SlotNotFoundError::new(slot))?,
//         })
//     }
//
//     fn code_by_hash(&mut self, _code_hash: H256) -> Result<Bytecode, Report> {
//         Err(eyre::eyre!("should never be called"))
//     }
// }
//
// fn is_precompile(address: &Address) -> bool {
//     address.le(&Address::from_str("0x0000000000000000000000000000000000000009").unwrap())
//         && address.gt(&Address::zero())
// }
//
// #[cfg(test)]
// mod tests {
//     use common::utils::hex_str_to_bytes;
//     use consensus::types::{primitives::ByteVector, ExecutionPayloadBellatrix};
//
//     use crate::rpc::mock_rpc::MockRpc;
//
//     use super::*;
//
//     fn get_client() -> ExecutionClient<MockRpc> {
//         ExecutionClient::new("testdata/").unwrap()
//     }
//
//     #[tokio::test]
//     async fn test_proof_db() {
//         // Construct proofdb params
//         let execution = get_client();
//         let address = Address::from_str("14f9D4aF749609c1438528C0Cce1cC3f6D411c47").unwrap();
//         let payload = ExecutionPayload::Bellatrix(ExecutionPayloadBellatrix {
//             state_root: ByteVector::try_from(
//                 hex_str_to_bytes(
//                     "0xaa02f5db2ee75e3da400d10f3c30e894b6016ce8a2501680380a907b6674ce0d",
//                 )
//                 .unwrap(),
//             )
//             .unwrap(),
//             ..ExecutionPayloadBellatrix::default()
//         });
//
//         let mut payloads = BTreeMap::new();
//         payloads.insert(7530933, payload.clone());
//
//         // Construct the proof database with the given client and payloads
//         let mut proof_db = ProofDB::new(Arc::new(execution), &payload, &payloads);
//
//         // Set the proof db accounts
//         let slot = U256::from(1337);
//         let mut accounts = HashMap::new();
//         let account = Account {
//             balance: U256::from(100),
//             code: hex_str_to_bytes("0x").unwrap(),
//             ..Default::default()
//         };
//         accounts.insert(address, account);
//         proof_db.set_accounts(accounts);
//
//         // Get the account from the proof database
//         let storage_proof = proof_db.storage(address, slot);
//
//         // Check that the storage proof correctly returns a slot not found error
//         let expected_err: eyre::Report = SlotNotFoundError::new(H256::from_uint(&slot)).into();
//         assert_eq!(
//             expected_err.to_string(),
//             storage_proof.unwrap_err().to_string()
//         );
//     }
// }
