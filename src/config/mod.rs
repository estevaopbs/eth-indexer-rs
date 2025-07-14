use serde::{Deserialize, Serialize};
use std::{env, fmt, fs};
use thiserror::Error;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AppConfig {
    pub database_url: String,
    pub eth_rpc_url: String,
    pub beacon_rpc_url: String, // Beacon Chain API URL (now mandatory)
    pub api_port: u16,
    pub start_block: Option<i64>, // Changed from u64 to i64 to support -1
    pub max_concurrent_requests: usize,
    pub blocks_per_batch: usize,
    pub sync_delay_seconds: Option<u32>, // Delay between sync attempts when already in sync
    pub block_fetch_interval_seconds: Option<u32>, // Polling interval for new blocks
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
            sync_delay_seconds: env::var("SYNC_DELAY_SECONDS")
                .ok()
                .and_then(|n| n.parse().ok()),
            block_fetch_interval_seconds: env::var("BLOCK_FETCH_INTERVAL_SECONDS")
                .ok()
                .and_then(|n| n.parse().ok()),
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
    /// If start_block is -1, it will be resolved to the latest network block
    pub async fn resolve_start_block(&mut self, db: &crate::database::DatabaseService, rpc: Option<&crate::rpc::RpcClient>) -> Result<(), ConfigError> {
        use tracing::{info, warn};

        // Get start_block from environment/config file
        let env_start_block = self.start_block;

        // Get start_block from database
        let db_start_block = db.get_start_block().await
            .map_err(|e| ConfigError::InvalidValue(format!("Failed to get start_block from database: {}", e)))?
            .map(|v| v as i64);

        match (db_start_block, env_start_block) {
            (Some(db_value), Some(env_value)) => {
                // Both database and environment have values
                if env_value == -1 {
                    // Special case: user wants to start from latest block
                    let resolved_block = self.resolve_latest_block(rpc).await?;
                    info!("Resolved START_BLOCK=-1 to latest network block: {}", resolved_block);
                    db.set_start_block(resolved_block as u64).await
                        .map_err(|e| ConfigError::InvalidValue(format!("Failed to save resolved start_block to database: {}", e)))?;
                    self.start_block = Some(resolved_block);
                } else if db_value != env_value {
                    warn!("Start block mismatch! Database has {}, environment/config has {}. Using database value.", db_value, env_value);
                    self.start_block = Some(db_value);
                } else {
                    info!("Start block consistent: {} (database and environment match)", db_value);
                    self.start_block = Some(db_value);
                }
            },
            (Some(db_value), None) => {
                // Only database has value
                info!("Using start block from database: {}", db_value);
                self.start_block = Some(db_value);
            },
            (None, Some(env_value)) => {
                // Only environment has value
                if env_value == -1 {
                    // Special case: user wants to start from latest block
                    let resolved_block = self.resolve_latest_block(rpc).await?;
                    info!("Resolved START_BLOCK=-1 to latest network block: {}", resolved_block);
                    db.set_start_block(resolved_block as u64).await
                        .map_err(|e| ConfigError::InvalidValue(format!("Failed to save resolved start_block to database: {}", e)))?;
                    self.start_block = Some(resolved_block);
                } else {
                    info!("Saving start block from environment to database: {}", env_value);
                    db.set_start_block(env_value as u64).await
                        .map_err(|e| ConfigError::InvalidValue(format!("Failed to save start_block to database: {}", e)))?;
                    self.start_block = Some(env_value);
                }
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

    /// Resolve START_BLOCK=-1 to the latest network block
    async fn resolve_latest_block(&self, rpc: Option<&crate::rpc::RpcClient>) -> Result<i64, ConfigError> {
        if let Some(rpc_client) = rpc {
            let latest_block = rpc_client.get_latest_block_number().await
                .map_err(|e| ConfigError::InvalidValue(format!("Failed to get latest block from RPC: {}", e)))?;
            Ok(latest_block as i64)
        } else {
            Err(ConfigError::InvalidValue(
                "RPC client is required to resolve START_BLOCK=-1".to_string()
            ))
        }
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
