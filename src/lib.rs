pub mod api;
pub mod beacon;
pub mod config;
pub mod database;
pub mod executor; // Generic RPC executor
pub mod health_cache; // Health cache service
pub mod historical; // Add historical module
pub mod indexer;
pub mod network_stats; // Add network stats module
pub mod rpc;
pub mod token_service; // Add token service module
pub mod web;

use crate::health_cache::HealthCacheService;
use crate::historical::HistoricalTransactionService;
use crate::network_stats::NetworkStatsService;
use crate::token_service::TokenService;
use anyhow::Result;
use beacon::BeaconClient;
use config::AppConfig;
use database::DatabaseService;
use indexer::IndexerService;
use rpc::RpcClient;
use std::sync::Arc;
use tracing::{error, info};

/// Represents the core application with all its services
#[derive(Clone)]
pub struct App {
    pub config: AppConfig,
    pub db: Arc<DatabaseService>,
    pub rpc: Arc<RpcClient>,
    pub beacon: Arc<BeaconClient>,
    pub indexer: Arc<IndexerService>,
    pub historical: Arc<HistoricalTransactionService>,
    pub network_stats: Arc<NetworkStatsService>,
    pub token_service: Arc<TokenService>,
    pub health_cache: Arc<HealthCacheService>,
}

impl App {
    /// Initialize a new application instance
    pub async fn init(mut config: AppConfig) -> Result<Self> {
        // Initialize database
        let db = Arc::new(DatabaseService::new(&config.database_url).await?);
        info!("Database initialized");

        // Initialize RPC client
        let rpc = Arc::new(RpcClient::new(&config.eth_rpc_url, config.clone())?);
        info!("RPC client connected to {}", config.eth_rpc_url);

        // Resolve start_block using database configuration and RPC (for -1 case)
        config.resolve_start_block(&db, Some(&rpc)).await?;

        // Initialize Beacon client with rate limiting
        let beacon = Arc::new(BeaconClient::new(&config.beacon_rpc_url, &config));
        info!("Beacon client connected to {}", config.beacon_rpc_url);

        // Initialize token service
        let token_service = Arc::new(TokenService::new(db.clone(), rpc.clone(), config.clone()));
        info!("Token service initialized");

        // Initialize indexer service with token service
        let indexer = Arc::new(IndexerService::with_token_service(
            db.clone(),
            rpc.clone(),
            beacon.clone(),
            token_service.clone(),
            config.clone(),
        ));
        info!("Indexer service initialized with token support");

        // Initialize historical transaction service
        let historical = Arc::new(HistoricalTransactionService::new(
            db.clone(),
            config.clone(),
        ));

        // Initialize historical data if start_block is configured
        if let Some(start_block) = config.start_block {
            if let Err(e) = historical.initialize(start_block).await {
                error!("Failed to initialize historical transaction service: {}", e);
            }
        }
        info!("Historical transaction service initialized");

        // Initialize network stats service
        let network_stats = Arc::new(NetworkStatsService::new(Arc::clone(&rpc)));

        // Start background updates for network stats
        network_stats.clone().start_background_updates().await;
        info!("Network stats service initialized");

        // Initialize health cache service
        let health_cache = Arc::new(HealthCacheService::new(Arc::clone(&rpc)));

        // Start background updates for health cache
        health_cache.clone().start_background_updates().await;
        info!("Health cache service initialized");

        Ok(Self {
            config,
            db,
            rpc,
            beacon,
            indexer,
            historical,
            network_stats,
            token_service,
            health_cache,
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
