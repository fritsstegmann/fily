mod fily;

use std::str::FromStr;

use dotenv::dotenv;
use serde::Deserialize;
use tokio::fs;
use tracing::Level;

#[derive(Deserialize)]
struct Config {
    log_level: String,
    fily: fily::Config,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    let config_file_config = String::from_utf8(fs::read("./config.toml").await?)?;

    let config: Config = toml::from_str(&config_file_config)?;

    tracing_subscriber::fmt()
        .with_max_level(Level::from_str(&config.log_level).unwrap())
        .with_level(true)
        .with_thread_names(true)
        .with_target(true)
        .init();

    let _ = fily::run(config.fily).await;

    Ok(())
}
