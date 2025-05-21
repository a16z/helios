use helios_core::client::HeliosClient;
use spec::Linea;

pub mod builder;
pub mod config;
pub mod consensus;
pub mod spec;

pub use builder::LineaClientBuilder;
pub type LineaClient = HeliosClient<Linea>;
