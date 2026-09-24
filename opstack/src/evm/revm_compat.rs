// op-revm 20 uses REVM 38. Keep that compatibility boundary here until it
// supports the REVM version needed for Ethereum's finalized Amsterdam rules.
use alloy::primitives::{Address, B256, U256};
use op_revm::revm as legacy;
use revm::context::result::{ExecutionResult, Output, ResultGas, SuccessReason};

pub(super) struct LegacyDb<'a, D>(pub &'a mut D);

impl<D: revm::Database> legacy::Database for LegacyDb<'_, D> {
    type Error = legacy::database_interface::ErasedError;

    fn basic(
        &mut self,
        address: Address,
    ) -> Result<Option<legacy::state::AccountInfo>, Self::Error> {
        self.0
            .basic(address)
            .map(|account| {
                account.map(|info| legacy::state::AccountInfo {
                    balance: info.balance,
                    nonce: info.nonce,
                    code_hash: info.code_hash,
                    code: info
                        .code
                        .map(|code| legacy::state::Bytecode::new_raw(code.original_bytes())),
                    ..Default::default()
                })
            })
            .map_err(Self::Error::new)
    }

    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        self.0.storage(address, index).map_err(Self::Error::new)
    }

    fn block_hash(&mut self, number: u64) -> Result<B256, Self::Error> {
        self.0.block_hash(number).map_err(Self::Error::new)
    }

    fn code_by_hash(&mut self, hash: B256) -> Result<legacy::state::Bytecode, Self::Error> {
        self.0
            .code_by_hash(hash)
            .map(|code| legacy::state::Bytecode::new_raw(code.original_bytes()))
            .map_err(Self::Error::new)
    }
}

pub(super) fn execution_result<H>(
    result: legacy::context::result::ExecutionResult<H>,
) -> ExecutionResult<H> {
    use legacy::context::result::{
        ExecutionResult as Old, Output as OldOutput, SuccessReason as OldReason,
    };
    let gas = result.gas();
    let gas = ResultGas::new_with_state_gas(
        gas.total_gas_spent(),
        gas.inner_refunded(),
        gas.floor_gas(),
        gas.state_gas_spent(),
    );
    match result {
        Old::Success {
            reason,
            logs,
            output,
            ..
        } => ExecutionResult::Success {
            reason: match reason {
                OldReason::Stop => SuccessReason::Stop,
                OldReason::Return => SuccessReason::Return,
                OldReason::SelfDestruct => SuccessReason::SelfDestruct,
            },
            gas,
            logs,
            output: match output {
                OldOutput::Call(data) => Output::Call(data),
                OldOutput::Create(data, address) => Output::Create(data, address),
            },
        },
        Old::Revert { output, .. } => ExecutionResult::Revert {
            gas,
            output,
            logs: vec![],
        },
        Old::Halt { reason, .. } => ExecutionResult::Halt {
            gas,
            reason,
            logs: vec![],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use legacy::{Context, ExecuteEvm};
    use op_revm::{DefaultOp, OpBuilder, OpTransaction};

    #[test]
    fn op_evm_reads_code_and_storage_from_shared_database() {
        let recipient = Address::repeat_byte(0x22);
        let mut db = revm::database::InMemoryDB::default();
        db.insert_account_info(
            recipient,
            revm::state::AccountInfo::default().with_code(revm::state::Bytecode::new_raw(
                alloy::primitives::bytes!("60005460005260206000f3"),
            )),
        );
        db.insert_account_storage(recipient, U256::ZERO, U256::from(42))
            .unwrap();
        let mut tx = OpTransaction::new(legacy::context::TxEnv {
            kind: recipient.into(),
            gas_limit: 100_000,
            chain_id: None,
            ..Default::default()
        });
        tx.enveloped_tx = Some(Default::default());
        let mut evm = Context::op()
            .with_db(LegacyDb(&mut db))
            .with_tx(tx)
            .build_op();
        let result = execution_result(evm.replay().unwrap().result);
        assert!(result.is_success(), "{result:?}");
        assert_eq!(
            result.output().unwrap().as_ref(),
            U256::from(42).to_be_bytes::<32>()
        );
        assert!(result.tx_gas_used() > 21_000);
    }
}
