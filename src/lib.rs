pub mod api;
pub mod beacon;
pub mod config;
pub mod database;
pub mod historical; // Add historical module
pub mod indexer;
pub mod network_stats; // Add network stats module
pub mod rpc;
pub mod web;

use anyhow::Result;
use beacon::BeaconClient;
use config::AppConfig;
use database::DatabaseService;
use indexer::IndexerService;
use rpc::RpcClient;
use std::sync::Arc;
use tracing::{error, info};
use crate::historical::HistoricalTransactionService;
use crate::network_stats::NetworkStatsService;

/// Represents the core application with all its services
pub struct App {
    pub config: AppConfig,
    pub db: Arc<DatabaseService>,
    pub rpc: Arc<RpcClient>,
    pub beacon: Arc<BeaconClient>,
    pub indexer: Arc<IndexerService>,
    pub historical: Arc<HistoricalTransactionService>,
    pub network_stats: Arc<NetworkStatsService>,
}

impl App {
    /// Initialize a new application instance
    pub async fn init() -> Result<Self> {
        // Load configuration
        let mut config = AppConfig::load()?;
        info!("Config loaded: {}", config);

        // Initialize database
        let db = Arc::new(DatabaseService::new(&config.database_url).await?);
        info!("Database initialized");

        // Initialize RPC client
        let rpc = Arc::new(RpcClient::new(&config.eth_rpc_url, config.clone())?);
        info!("RPC client connected to {}", config.eth_rpc_url);

        // Resolve start_block using database configuration and RPC (for -1 case)
        config.resolve_start_block(&db, Some(&rpc)).await?;

        // Initialize Beacon client
        let beacon = Arc::new(BeaconClient::new(&config.beacon_rpc_url));
        info!("Beacon client connected to {}", config.beacon_rpc_url);

        // Initialize indexer service
        let indexer = Arc::new(IndexerService::new(
            db.clone(),
            rpc.clone(),
            beacon.clone(),
            config.clone(),
        ));
        info!("Indexer service initialized");

        // Initialize historical transaction service
        let historical = Arc::new(HistoricalTransactionService::new(db.clone(), config.clone()));
        
        // Initialize historical data if start_block is configured
        if let Some(start_block) = config.start_block {
            if let Err(e) = historical.initialize(start_block).await {
                error!("Failed to initialize historical transaction service: {}", e);
            }
        }
        info!("Historical transaction service initialized");

        // Initialize network stats service
        let network_stats = Arc::new(NetworkStatsService::new(
            Arc::clone(&rpc),
            Arc::clone(&historical),
        ));
        
        // Start background updates for network stats
        network_stats.clone().start_background_updates().await;
        info!("Network stats service initialized");

        Ok(Self {
            config,
            db,
            rpc,
            beacon,
            indexer,
            historical,
            network_stats,
        })
    }

    /// Start all application services
    pub async fn start(&self) -> Result<()> {
        // Start the indexer process
        let indexer = self.indexer.clone();
        tokio::spawn(async move {
            if let Err(e) = indexer.start_service().await {
                error!("Indexer service error: {}", e);
            }
        });

        info!("Application started successfully");
        Ok(())
    }
}
