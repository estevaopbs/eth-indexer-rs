use serde::{Deserialize, Serialize};
use std::{env, fmt, fs};
use thiserror::Error;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AppConfig {
    pub database_url: String,
    pub eth_rpc_url: String,
    pub beacon_rpc_url: String, // Beacon Chain API URL (now mandatory)
    pub api_port: u16,
    pub start_block: Option<u64>,
    pub max_concurrent_requests: usize,
    pub blocks_per_batch: usize,
    pub log_level: String,
    pub bigquery_service_account_path: Option<String>,
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to load environment variables: {0}")]
    EnvError(#[from] dotenvy::Error),

    #[error("Missing required environment variable: {0}")]
    MissingEnv(String),

    #[error("Invalid configuration value: {0}")]
    InvalidValue(String),
}

impl AppConfig {
    /// Load configuration from environment variables
    pub fn load() -> Result<Self, ConfigError> {
        // Load .env file if present (ignore error if not found)
        let _ = dotenvy::dotenv();

        // Initialize with defaults
        let config = Self {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:./data/indexer.db".to_string()),
            eth_rpc_url: env::var("ETH_RPC_URL")
                .unwrap_or_else(|_| "https://mainnet.infura.io/v3/your-infura-key".to_string()),
            beacon_rpc_url: env::var("BEACON_RPC_URL")
                .map_err(|_| ConfigError::MissingEnv("BEACON_RPC_URL".to_string()))?, // Now mandatory
            api_port: env::var("API_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
            start_block: env::var("START_BLOCK").ok().and_then(|b| b.parse().ok()),
            max_concurrent_requests: env::var("MAX_CONCURRENT_REQUESTS")
                .ok()
                .and_then(|n| n.parse().ok())
                .unwrap_or(5),
            blocks_per_batch: env::var("BLOCKS_PER_BATCH")
                .ok()
                .and_then(|n| n.parse().ok())
                .unwrap_or(10),
            log_level: env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
            bigquery_service_account_path: env::var("BIGQUERY_SERVICE_ACCOUNT_PATH").ok(),
        };

        // Ensure data directory exists
        if let Some(path) = config.database_url.strip_prefix("sqlite:") {
            let path = path.trim_start_matches("/");
            if let Some(dir) = std::path::Path::new(path).parent() {
                fs::create_dir_all(dir).map_err(|e| {
                    ConfigError::InvalidValue(format!(
                        "Failed to create directory for database: {}",
                        e
                    ))
                })?;
            }
        }

        // Validate RPC URLs
        if !config.eth_rpc_url.starts_with("http") && !config.eth_rpc_url.starts_with("ws") {
            return Err(ConfigError::InvalidValue(
                "ETH_RPC_URL must start with http:// or ws://".to_string(),
            ));
        }

        if !config.beacon_rpc_url.starts_with("http") && !config.beacon_rpc_url.starts_with("ws") {
            return Err(ConfigError::InvalidValue(
                "BEACON_RPC_URL must start with http:// or ws://".to_string(),
            ));
        }

        Ok(config)
    }
}

impl fmt::Display for AppConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AppConfig {{ database_url: {}, eth_rpc_url: {}, beacon_rpc_url: {}, api_port: {}, start_block: {:?} }}",
            self.database_url, self.eth_rpc_url, self.beacon_rpc_url, self.api_port, self.start_block
        )
    }
}
