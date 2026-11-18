use dirs::home_dir;
use eyre::Result;

use helios::ethereum::config::{cli::CliConfig, Config};

#[tokio::main]
async fn main() -> Result<()> {
    // Load the config from the global config file
    let home = home_dir().ok_or_else(|| eyre::eyre!("Could not find home directory"))?;
    let config_path = home.join(".helios/helios.toml");
    let config = Config::from_file(&config_path, "mainnet", &CliConfig::default());
    // Do not Debug-print Config: Url fields embed the full string, so RPC URLs
    // that carry API keys (for example Alchemy `/v2/<key>`) would leak to logs.
    println!(
        "Loaded Helios config for 'mainnet' from {} (chain id {})",
        config_path.display(),
        config.chain.chain_id
    );

    Ok(())
}
