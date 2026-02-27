#![warn(missing_debug_implementations, rust_2018_idioms, unreachable_pub)]
#![deny(rustdoc::broken_intra_doc_links)]
#![doc(test(
    no_crate_inject,
    attr(deny(warnings, rust_2018_idioms), allow(dead_code, unused_variables))
))]

//! # Ethereum light client written in Rust.
//!
//! > Helios is a fully trustless, efficient, and portable Ethereum light client written in Rust.
//!
//! Helios converts an untrusted centralized RPC endpoint into a safe unmanipulable local RPC for its users. It syncs in seconds, requires no storage, and is lightweight enough to run on mobile devices.
//!
//! The entire size of Helios's binary is 13Mb and compiles into WebAssembly. This makes it a perfect target to embed directly inside wallets and dapps.
//!
//! Examples on how you can use helios can be found in the [`examples` directory of the repository](https://github.com/a16z/helios/tree/master/examples) and in the `tests/` directories of each crate.
//!

pub mod common {
    pub use helios_common::*;
}

pub mod core {
    pub use helios_core::*;
}

pub mod ethereum {
    pub use helios_ethereum::*;
}

pub mod opstack {
    pub use helios_opstack::*;
}

pub mod mantle {
    pub use helios_mantle::*;
}
