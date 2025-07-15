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

    // Worker and Queue Configuration
    pub max_concurrent_blocks: usize, // Max blocks being processed simultaneously
    pub worker_pool_size: usize,      // Number of worker threads in the pool
    pub max_concurrent_tx_receipts: usize, // Max transaction receipts fetched simultaneously
    pub block_queue_size_multiplier: usize, // Queue size = worker_pool_size * multiplier

    // RPC Rate Limiting Configuration
    pub eth_rpc_min_interval_ms: u64, // Min interval between ETH RPC requests (ms)
    pub beacon_rpc_min_interval_ms: u64, // Min interval between Beacon RPC requests (ms)
    pub eth_rpc_max_concurrent: usize, // Max concurrent ETH RPC requests
    pub beacon_rpc_max_concurrent: usize, // Max concurrent Beacon RPC requests

    // Batch Processing Configuration
    pub account_batch_size: usize, // Batch size for account balance fetching
    pub rpc_batch_size: usize,     // Batch size for RPC calls
    pub max_concurrent_balance_fetches: usize, // Max concurrent balance fetch operations

    // Token Service Configuration
    pub token_balance_update_interval_ms: u64, // Interval between token balance updates (ms)
    pub token_refresh_interval_ms: u64,        // Interval between token refresh operations (ms)

    // Timing Configuration
    pub sync_delay_seconds: Option<u32>, // Delay between sync attempts when already in sync
    pub block_fetch_interval_seconds: Option<u32>, // Polling interval for new blocks
    pub worker_timeout_seconds: u64,     // Timeout for workers waiting for blocks (seconds)
    pub bigquery_service_account_path: Option<String>,

    // Logging Configuration
    pub log_level: String, // Log level for tracing (e.g., "info", "debug", "error")
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

            // Worker and Queue Configuration
            max_concurrent_blocks: env::var("MAX_CONCURRENT_BLOCKS")
                .ok()
                .and_then(|n| n.parse().ok())
                .unwrap_or(10),
            worker_pool_size: env::var("WORKER_POOL_SIZE")
                .ok()
                .and_then(|n| n.parse().ok())
                .unwrap_or(8),
            max_concurrent_tx_receipts: env::var("MAX_CONCURRENT_TX_RECEIPTS")
                .ok()
                .and_then(|n| n.parse().ok())
                .unwrap_or(50),
            block_queue_size_multiplier: env::var("BLOCK_QUEUE_SIZE_MULTIPLIER")
                .ok()
                .and_then(|n| n.parse().ok())
                .unwrap_or(4),

            // RPC Rate Limiting Configuration
            eth_rpc_min_interval_ms: env::var("ETH_RPC_MIN_INTERVAL_MS")
                .ok()
                .and_then(|n| n.parse().ok())
                .unwrap_or(0),
            beacon_rpc_min_interval_ms: env::var("BEACON_RPC_MIN_INTERVAL_MS")
                .ok()
                .and_then(|n| n.parse().ok())
                .unwrap_or(0),
            eth_rpc_max_concurrent: env::var("ETH_RPC_MAX_CONCURRENT")
                .ok()
                .and_then(|n| n.parse().ok())
                .unwrap_or(20),
            beacon_rpc_max_concurrent: env::var("BEACON_RPC_MAX_CONCURRENT")
                .ok()
                .and_then(|n| n.parse().ok())
                .unwrap_or(10),

            // Batch Processing Configuration
            account_batch_size: env::var("ACCOUNT_BATCH_SIZE")
                .ok()
                .and_then(|n| n.parse().ok())
                .unwrap_or(100),
            rpc_batch_size: env::var("RPC_BATCH_SIZE")
                .ok()
                .and_then(|n| n.parse().ok())
                .unwrap_or(10),
            max_concurrent_balance_fetches: env::var("MAX_CONCURRENT_BALANCE_FETCHES")
                .ok()
                .and_then(|n| n.parse().ok())
                .unwrap_or(10),

            // Token Service Configuration
            token_balance_update_interval_ms: env::var("TOKEN_BALANCE_UPDATE_INTERVAL_MS")
                .ok()
                .and_then(|n| n.parse().ok())
                .unwrap_or(10),
            token_refresh_interval_ms: env::var("TOKEN_REFRESH_INTERVAL_MS")
                .ok()
                .and_then(|n| n.parse().ok())
                .unwrap_or(50),

            // Timing Configuration
            sync_delay_seconds: env::var("SYNC_DELAY_SECONDS")
                .ok()
                .and_then(|n| n.parse().ok()),
            block_fetch_interval_seconds: env::var("BLOCK_FETCH_INTERVAL_SECONDS")
                .ok()
                .and_then(|n| n.parse().ok()),
            worker_timeout_seconds: env::var("WORKER_TIMEOUT_SECONDS")
                .ok()
                .and_then(|n| n.parse().ok())
                .unwrap_or(30),
            bigquery_service_account_path: env::var("BIGQUERY_SERVICE_ACCOUNT_PATH").ok(),
            log_level: env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
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

    /// Resolve the start_block using database cache and environment variables
    /// Database cache takes precedence. If cache exists, env values are ignored (except for warnings).
    /// Negative values in env represent relative positions: -1=latest, -2=second latest, etc.
    pub async fn resolve_start_block(
        &mut self,
        db: &crate::database::DatabaseService,
        rpc: Option<&crate::rpc::RpcClient>,
    ) -> Result<(), ConfigError> {
        use tracing::{info, warn};

        // Get start_block from environment/config file
        let env_start_block = self.start_block;

        // Check if database cache already exists
        let db_cache = db.get_start_block_cache().await.map_err(|e| {
            ConfigError::InvalidValue(format!(
                "Failed to get start_block cache from database: {}",
                e
            ))
        })?;

        match db_cache {
            Some((db_start_block, _)) => {
                // Database cache exists - use it and ignore env (except for warnings)
                info!("Using start block from database cache: {}", db_start_block);

                if let Some(env_value) = env_start_block {
                    if env_value >= 0 && env_value != db_start_block as i64 {
                        warn!("Start block mismatch! Database cache has {}, environment has {}. Using database value.", db_start_block, env_value);
                    }
                    // Note: Negative env values are ignored when cache exists (no warning)
                }

                self.start_block = Some(db_start_block as i64);
            }
            None => {
                // No database cache - initialize based on environment
                let resolved_start_block = match env_start_block {
                    Some(env_value) if env_value < 0 => {
                        // Negative value: resolve relative to latest block
                        let latest_block = self.resolve_latest_block(rpc).await?;
                        let relative_block = latest_block + env_value; // env_value is negative
                        let final_block = if relative_block < 0 {
                            0
                        } else {
                            relative_block
                        };
                        info!(
                            "Resolved START_BLOCK={} to block: {} (latest was {})",
                            env_value, final_block, latest_block
                        );
                        final_block as u64
                    }
                    Some(env_value) if env_value >= 0 => {
                        // Positive value: use as-is
                        info!("Using start block from environment: {}", env_value);
                        env_value as u64
                    }
                    _ => {
                        // No environment value or invalid: use default of 0
                        info!("No start block configured, using default: 0");
                        0
                    }
                };

                // Initialize the database cache
                db.init_start_block_cache(resolved_start_block)
                    .await
                    .map_err(|e| {
                        ConfigError::InvalidValue(format!(
                            "Failed to initialize start_block cache: {}",
                            e
                        ))
                    })?;

                self.start_block = Some(resolved_start_block as i64);
            }
        }

        Ok(())
    }

    /// Resolve START_BLOCK=-1 to the latest network block
    async fn resolve_latest_block(
        &self,
        rpc: Option<&crate::rpc::RpcClient>,
    ) -> Result<i64, ConfigError> {
        if let Some(rpc_client) = rpc {
            let latest_block = rpc_client.get_latest_block_number().await.map_err(|e| {
                ConfigError::InvalidValue(format!("Failed to get latest block from RPC: {}", e))
            })?;
            Ok(latest_block as i64)
        } else {
            Err(ConfigError::InvalidValue(
                "RPC client is required to resolve START_BLOCK=-1".to_string(),
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
