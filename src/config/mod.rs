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

    /// Resolve the start_block using database configuration and environment variables
    /// Returns the final start_block value to use, checking database first, then env vars
    pub async fn resolve_start_block(&mut self, db: &crate::database::DatabaseService) -> Result<(), ConfigError> {
        use tracing::{info, warn};

        // Get start_block from environment/config file
        let env_start_block = self.start_block;

        // Get start_block from database
        let db_start_block = db.get_start_block().await
            .map_err(|e| ConfigError::InvalidValue(format!("Failed to get start_block from database: {}", e)))?;

        match (db_start_block, env_start_block) {
            (Some(db_value), Some(env_value)) => {
                // Both database and environment have values
                if db_value != env_value {
                    warn!("Start block mismatch! Database has {}, environment/config has {}. Using database value.", db_value, env_value);
                } else {
                    info!("Start block consistent: {} (database and environment match)", db_value);
                }
                self.start_block = Some(db_value);
            },
            (Some(db_value), None) => {
                // Only database has value
                info!("Using start block from database: {}", db_value);
                self.start_block = Some(db_value);
            },
            (None, Some(env_value)) => {
                // Only environment has value, save to database
                info!("Saving start block from environment to database: {}", env_value);
                db.set_start_block(env_value).await
                    .map_err(|e| ConfigError::InvalidValue(format!("Failed to save start_block to database: {}", e)))?;
                self.start_block = Some(env_value);
            },
            (None, None) => {
                // Neither has value, use default of 0
                info!("No start block configured, using default: 0");
                db.set_start_block(0).await
                    .map_err(|e| ConfigError::InvalidValue(format!("Failed to save default start_block to database: {}", e)))?;
                self.start_block = Some(0);
            }
        }

        Ok(())
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
