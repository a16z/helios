use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::{
    path::PathBuf,
    process::exit,
    str::FromStr,
    sync::{Arc, Mutex},
};

use alloy::primitives::hex;
use alloy::primitives::B256;
use clap::{Args, Parser, Subcommand};
use dirs::home_dir;
use eyre::Result;
use figment::providers::Serialized;
use figment::value::Value;
use futures::executor::block_on;
use tracing::{error, info};
use tracing_subscriber::filter::{EnvFilter, LevelFilter};
use tracing_subscriber::FmtSubscriber;
use url::Url;

use helios_common::network_spec::NetworkSpec;
use helios_core::client::Client;
use helios_core::consensus::Consensus;
use helios_ethereum::config::{cli::CliConfig, Config as EthereumConfig};
use helios_ethereum::database::FileDB;
use helios_ethereum::{EthereumClient, EthereumClientBuilder};
use helios_opstack::{config::Config as OpStackConfig, OpStackClient, OpStackClientBuilder};
use helios_linea::{config::Config as LineaConfig, types::LineaClient, builder::LineaClientBuilder, config::CliConfig as LineaCliConfig};
use tracing::{error, info};
use tracing_subscriber::filter::{EnvFilter, LevelFilter};
use tracing_subscriber::FmtSubscriber;
use url::Url;

#[tokio::main]
async fn main() -> Result<()> {
    enable_tracer();

    let cli = Cli::parse();
    match cli.command {
        Command::Ethereum(ethereum) => {
            let mut client = ethereum.make_client();
            start_client(&mut client).await;
            register_shutdown_handler(client);
        }
        Command::OpStack(opstack) => {
            let mut client = opstack.make_client();
            start_client(&mut client).await;
            register_shutdown_handler(client);
        }
        Command::Linea(linea) => {
            let mut client = linea.make_client();
            start_client(&mut client).await;
            register_shutdown_handler(client);
        }
    }

    std::future::pending().await
}

fn enable_tracer() {
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env()
        .expect("invalid env filter");

    let subscriber = FmtSubscriber::builder()
        .with_env_filter(env_filter)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("subscriber set failed");
}

async fn start_client<N: NetworkSpec, C: Consensus<N::BlockResponse>>(client: &mut Client<N, C>) {
    if let Err(err) = client.start().await {
        error!(target: "helios::runner", error = %err);
        exit(1);
    }
}

fn register_shutdown_handler<N: NetworkSpec, C: Consensus<N::BlockResponse>>(client: Client<N, C>) {
    let client = Arc::new(client);
    let shutdown_counter = Arc::new(Mutex::new(0));

    ctrlc::set_handler(move || {
        let mut counter = shutdown_counter.lock().unwrap();
        *counter += 1;

        let counter_value = *counter;

        if counter_value == 3 {
            info!(target: "helios::runner", "forced shutdown");
            exit(0);
        }

        info!(
            target: "helios::runner",
            "shutting down... press ctrl-c {} more times to force quit",
            3 - counter_value
        );

        if counter_value == 1 {
            let client = client.clone();
            std::thread::spawn(move || {
                block_on(client.shutdown());
                exit(0);
            });
        }
    })
    .expect("could not register shutdown handler");
}

#[derive(Parser)]
#[clap(version, about)]
/// Helios is a fast, secure, and portable multichain light client
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[clap(name = "ethereum")]
    Ethereum(EthereumArgs),
    #[clap(name = "opstack")]
    OpStack(OpStackArgs),
    #[clap(name = "linea")]
    Linea(LineaArgs),
}

#[derive(Args)]
struct EthereumArgs {
    #[clap(short, long, default_value = "mainnet")]
    network: String,
    #[clap(short = 'b', long, env)]
    rpc_bind_ip: Option<IpAddr>,
    #[clap(short = 'p', long, env)]
    rpc_port: Option<u16>,
    #[clap(short = 'w', long, env)]
    checkpoint: Option<B256>,
    #[clap(short, long, env, value_parser = parse_url)]
    execution_rpc: Option<Url>,
    #[clap(long, env, value_parser = parse_url)]
    execution_verifiable_api: Option<Url>,
    #[clap(short, long, env, value_parser = parse_url)]
    consensus_rpc: Option<Url>,
    #[clap(short, long, env)]
    data_dir: Option<String>,
    #[clap(short = 'f', long, env)]
    fallback: Option<String>,
    #[clap(short = 'l', long, env)]
    load_external_fallback: bool,
    #[clap(short = 's', long, env)]
    strict_checkpoint_age: bool,
}

impl EthereumArgs {
    fn make_client(&self) -> EthereumClient<FileDB> {
        let config_path = home_dir().unwrap().join(".helios/helios.toml");
        let cli_config = self.as_cli_config();
        let config = EthereumConfig::from_file(&config_path, &self.network, &cli_config);

        match EthereumClientBuilder::new()
            .config(config)
            .build::<FileDB>()
        {
            Ok(client) => client,
            Err(err) => {
                error!(target: "helios::runner", error = %err);
                exit(1);
            }
        }
    }

    fn as_cli_config(&self) -> CliConfig {
        CliConfig {
            checkpoint: self.checkpoint,
            execution_rpc: self.execution_rpc.clone(),
            execution_verifiable_api: self.execution_verifiable_api.clone(),
            consensus_rpc: self.consensus_rpc.clone(),
            data_dir: self
                .data_dir
                .as_ref()
                .map(|s| PathBuf::from_str(s).expect("cannot find data dir")),
            rpc_bind_ip: self.rpc_bind_ip,
            rpc_port: self.rpc_port,
            fallback: self.fallback.clone(),
            load_external_fallback: true_or_none(self.load_external_fallback),
            strict_checkpoint_age: true_or_none(self.strict_checkpoint_age),
        }
    }
}

#[derive(Args, Debug)]
struct OpStackArgs {
    #[clap(short, long)]
    network: String,
    #[clap(short = 'b', long, env, default_value = "127.0.0.1")]
    rpc_bind_ip: Option<IpAddr>,
    #[clap(short = 'p', long, env, default_value = "8545")]
    rpc_port: Option<u16>,
    #[clap(short, long, env, value_parser = parse_url)]
    execution_rpc: Option<Url>,
    #[clap(long, env, value_parser = parse_url)]
    execution_verifiable_api: Option<Url>,
    #[clap(short, long, env, value_parser = parse_url)]
    consensus_rpc: Option<Url>,
    #[clap(
        short = 'w',
        long = "ethereum-checkpoint",
        env = "ETHEREUM_CHECKPOINT",
        help = "Set custom weak subjectivity checkpoint for chosen Ethereum network. Helios uses this to sync and trustlessly fetch the correct unsafe signer address used by <NETWORK>"
    )]
    checkpoint: Option<B256>,
    #[clap(
        short = 'l',
        long = "ethereum-load-external-fallback",
        env = "ETHEREUM_LOAD_EXTERNAL_FALLBACK",
        help = "Enable fallback for weak subjectivity checkpoint. Use if --ethereum-checkpoint fails."
    )]
    load_external_fallback: bool,
}

impl OpStackArgs {
    fn make_client(&self) -> OpStackClient {
        let config_path = home_dir().unwrap().join(".helios/helios.toml");
        let cli_provider = self.as_provider();
        let config = OpStackConfig::from_file(&config_path, &self.network, cli_provider);

        match OpStackClientBuilder::new().config(config).build() {
            Ok(client) => client,
            Err(err) => {
                error!(target: "helios::runner", error = %err);
                exit(1);
            }
        }
    }

    fn as_provider(&self) -> Serialized<HashMap<&str, Value>> {
        let mut user_dict = HashMap::new();

        if let Some(rpc) = &self.execution_rpc {
            user_dict.insert("execution_rpc", Value::from(rpc.to_string()));
        }

        if let Some(api) = &self.execution_verifiable_api {
            user_dict.insert("execution_verifiable_api", Value::from(api.to_string()));
        }

        if let Some(rpc) = &self.consensus_rpc {
            user_dict.insert("consensus_rpc", Value::from(rpc.to_string()));
        }

        if self.rpc_bind_ip.is_some() && self.rpc_port.is_some() {
            let rpc_socket = SocketAddr::new(self.rpc_bind_ip.unwrap(), self.rpc_port.unwrap());
            user_dict.insert("rpc_socket", Value::from(rpc_socket.to_string()));
        }

        if let Some(ip) = self.rpc_bind_ip {
            user_dict.insert("rpc_bind_ip", Value::from(ip.to_string()));
        }

        if let Some(port) = self.rpc_port {
            user_dict.insert("rpc_port", Value::from(port));
        }

        if self.load_external_fallback {
            user_dict.insert("load_external_fallback", Value::from(true));
        }

        if let Some(checkpoint) = self.checkpoint {
            user_dict.insert("checkpoint", Value::from(hex::encode(checkpoint)));
        }

        Serialized::from(user_dict, &self.network)
    }
}

#[derive(Args, Debug)]
struct LineaArgs {
    #[clap(short, long, default_value = "mainnet")]
    network: String,
    #[clap(short = 'b', long, env)]
    rpc_bind_ip: Option<IpAddr>,
    #[clap(short = 'p', long, env)]
    rpc_port: Option<u16>,
    #[clap(short, long, env, value_parser = parse_url)]
    execution_rpc: Option<Url>,
}

impl LineaArgs {
    fn make_client(&self) -> LineaClient {
        let config_path = home_dir().unwrap().join(".helios/helios.toml");
        let cli_config = self.as_cli_config();
        let config = LineaConfig::from_file(&config_path, &self.network, &cli_config);

        match LineaClientBuilder::new().config(config).build() {
            Ok(client) => client,
            Err(err) => {
                error!(target: "helios::runner", error = %err);
                exit(1);
            }
        }
    }

    fn as_cli_config(&self) -> LineaCliConfig {
        LineaCliConfig {
            execution_rpc: self.execution_rpc.clone(),
            rpc_bind_ip: self.rpc_bind_ip,
            rpc_port: self.rpc_port,
        }
    }
}

fn true_or_none(b: bool) -> Option<bool> {
    if b {
        Some(b)
    } else {
        None
    }
}

fn parse_url(s: &str) -> Result<Url, url::ParseError> {
    Url::parse(s)
}
