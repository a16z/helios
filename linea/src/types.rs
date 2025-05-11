use helios_core::client::Client;

use crate::{consensus::ConsensusClient, spec::Linea};

pub type LineaClient = Client<Linea, ConsensusClient>;
