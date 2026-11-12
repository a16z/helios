use dirs::home_dir;
use eyre::Result;

use helios::ethereum::config::{cli::CliConfig, Config};

#[tokio::main]
async fn main() -> Result<()> {
    // Load the config from the global config file
    let home = home_dir().ok_or_else(|| eyre::eyre!("Could not find home directory"))?;
    let config_path = home.join(".helios/helios.toml");
    let config = Config::from_file(&config_path, "mainnet", &CliConfig::default());
    // Do not print the whole config: url::Url renders the full URL in its
    // Debug output, which can expose RPC API keys in stdout/CI logs.
    println!("Constructed config successfully");

    Ok(())
}
