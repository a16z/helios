use helios_core::client::Client;

use crate::consensus::ConsensusClient;
use crate::spec::Linea;

pub type LineaClient = Client<Linea, ConsensusClient>;