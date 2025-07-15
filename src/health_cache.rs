use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time;
use tracing::{debug, error, info};

use crate::rpc::RpcClient;

/// Cache for health check information
#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub rpc_connected: bool,
    pub last_checked: Instant,
}

impl Default for HealthStatus {
    fn default() -> Self {
        Self {
            rpc_connected: false,
            last_checked: Instant::now(),
        }
    }
}

/// Health cache service that periodically checks RPC connection
pub struct HealthCacheService {
    rpc: Arc<RpcClient>,
    cached_status: Arc<RwLock<HealthStatus>>,
    cache_duration: Duration,
}

impl HealthCacheService {
    pub fn new(rpc: Arc<RpcClient>) -> Self {
        Self {
            rpc,
            cached_status: Arc::new(RwLock::new(HealthStatus::default())),
            cache_duration: Duration::from_secs(60), // 60 seconds cache
        }
    }

    /// Start the background service to periodically update health status
    pub async fn start_background_updates(self: Arc<Self>) {
        let service = Arc::clone(&self);
        tokio::spawn(async move {
            info!("Health cache service starting background updates");
            let mut interval = time::interval(service.cache_duration);

            // Perform initial check
            service.update_health_status().await;

            loop {
                interval.tick().await;
                service.update_health_status().await;
            }
        });
    }

    /// Update the cached health status
    async fn update_health_status(&self) {
        debug!("Updating health status cache");

        let is_connected = self.rpc.check_connection().await.unwrap_or(false);

        let new_status = HealthStatus {
            rpc_connected: is_connected,
            last_checked: Instant::now(),
        };

        {
            let mut cached = self.cached_status.write().await;
            *cached = new_status;
        }

        debug!("Health status updated: rpc_connected={}", is_connected);
    }

    /// Get the cached health status
    pub async fn get_health_status(&self) -> HealthStatus {
        let cached = self.cached_status.read().await;
        cached.clone()
    }

    /// Force an immediate health status update (useful for startup)
    pub async fn force_update(&self) {
        self.update_health_status().await;
    }
}
