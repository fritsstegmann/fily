mod fily;
mod config;

use std::str::FromStr;
use std::env;

use dotenv::dotenv;
use tracing::Level;
use config::ConfigLoader;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file if present (for development)
    dotenv().ok();

    // Check for help flag
    if env::args().any(|arg| arg == "--help" || arg == "-h" || arg == "help") {
        ConfigLoader::print_help();
        return Ok(());
    }

    // Load configuration from environment variables
    let config = ConfigLoader::load()?;
    
    // Validate configuration
    ConfigLoader::validate(&config)?;

    // Initialize tracing with configured log level
    tracing_subscriber::fmt()
        .with_max_level(Level::from_str(&config.log_level).unwrap())
        .with_level(true)
        .with_thread_names(true)
        .with_target(true)
        .init();

    // Run the server
    fily::run(config).await
}
