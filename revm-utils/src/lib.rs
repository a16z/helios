pub mod proof_db;
pub mod types;

/// Maximum shared EVM memory per execution, in bytes.
pub const EVM_MEMORY_LIMIT: u64 = if usize::BITS == 32 {
    1 << 30
} else {
    u32::MAX as u64
};
