#![warn(missing_debug_implementations, rust_2018_idioms, unreachable_pub)]
#![deny(rustdoc::broken_intra_doc_links)]
#![doc(test(
    no_crate_inject,
    attr(deny(warnings, rust_2018_idioms), allow(dead_code, unused_variables))
))]

//! # Ethereum light client written in Rust.
//!
//! > helios is a fully trustless, efficient, and portable Ethereum light client written in Rust.
//!
//! Helios converts an untrusted centralized RPC endpoint into a safe unmanipulable local RPC for its users. It syncs in seconds, requires no storage, and is lightweight enough to run on mobile devices.
//!
//! The entire size of Helios's binary is 13Mb and should be easy to compile into WebAssembly. This makes it a perfect target to embed directly inside wallets and dapps.
//!
//! ## Quickstart: `prelude`
//!
//! The prelude imports all the necessary data types and traits from helios. Use this to quickly bootstrap a new project.
//!
//! ```no_run
//! # #[allow(unused)]
//! use helios::prelude::*;
//! ```
//!
//! Examples on how you can use the types imported by the prelude can be found in
//! the [`examples` directory of the repository](https://github.com/a16z/helios/tree/master/examples)
//! and in the `tests/` directories of each crate.
//!
//! ## Breakdown of exported helios modules
//!
//! ### `client`
//!
//! The `client` module exports three main types: `Client`, `ClientBuilder`, and `FileDB`.
//!
//! `ClientBuilder` is a builder for the `Client` type. It allows you to configure the client using the fluent builder pattern.
//!
//! `Client` serves Ethereum RPC endpoints locally that call a node on the backend.
//!
//! Finally, the `FileDB` type is a simple local database. It is used by the `Client` to store checkpoint data.
//!
//! ### `config`
//!
//! The `config` module provides the configuration types for all of helios. It is used by the `ClientBuilder` to configure the `Client`.
//!
//! ### `types`
//!
//! Generic types used across helios.
//!
//! ### `errors`
//!
//! Errors used across helios.

pub mod consensus {
    pub use consensus::*;
}

pub mod config {
    pub use config::{checkpoints, networks, Config};
}

pub mod types {
    pub use common::config::types::*;
    pub use common::consensus::types::*;
    pub use common::execution::types::*;
    pub use common::types::{Block, BlockTag, Transactions};
    pub use execution::types::{Account, CallOpts};
}

pub mod client {
    pub use consensus::database::*;
}

pub mod prelude {
    pub use crate::client::*;
    pub use crate::common::*;
    pub use crate::config::*;
    pub use crate::errors::*;
    pub use crate::types::*;
}

pub mod errors {
    pub use common::consensus::errors::*;
    pub use common::errors::*;
    pub use execution::errors::*;
}

pub mod constants {
    pub use common::execution::constants::*;
}

pub mod common {
    pub use common::*;
}
