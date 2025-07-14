use anyhow::{Context, Result};
use regex::Regex;
use reqwest::Client;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::time;
use tracing::{debug, error, info, warn};

use crate::historical::HistoricalTransactionService;
use crate::rpc::RpcClient;

/// Service for fetching and caching network-wide statistics
pub struct NetworkStatsService {
    client: Client,
    rpc: Arc<RpcClient>,
    historical: Arc<HistoricalTransactionService>,
    cached_network_accounts: Arc<RwLock<Option<(u64, Instant)>>>,
    cached_latest_block: Arc<RwLock<Option<(u64, Instant)>>>,
}

impl NetworkStatsService {
    const CACHE_DURATION: Duration = Duration::from_secs(43200); // 12 hours cache
    const ETHERSCAN_URL: &'static str = "https://etherscan.io/chart/address";

    pub fn new(rpc: Arc<RpcClient>, historical: Arc<HistoricalTransactionService>) -> Self {
        let client = Client::builder()
            .user_agent(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:140.0) Gecko/20100101 Firefox/140.0",
            )
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap();

        Self {
            client,
            rpc,
            historical,
            cached_network_accounts: Arc::new(RwLock::new(None)),
            cached_latest_block: Arc::new(RwLock::new(None)),
        }
    }

    /// Start the background service to periodically update network stats
    pub async fn start_background_updates(self: Arc<Self>) {
        let service = Arc::clone(&self);
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(30)); // Update every 30 seconds

            loop {
                interval.tick().await;

                // Update latest block
                if let Err(e) = service.update_latest_block().await {
                    warn!("Failed to update latest block: {}", e);
                }

                // Update network accounts (every 12 hours)
                if service.should_update_accounts() {
                    if let Err(e) = service.update_network_accounts().await {
                        warn!("Failed to update network accounts: {}", e);
                    }
                }
            }
        });
    }

    /// Get the latest network block number
    pub async fn get_latest_network_block(&self) -> Option<u64> {
        // Check cache first
        if let Ok(guard) = self.cached_latest_block.read() {
            if let Some((value, timestamp)) = *guard {
                if timestamp.elapsed() < Duration::from_secs(10) {
                    // Very short cache for block numbers
                    return Some(value);
                }
            }
        }

        // Fetch from RPC
        match self.rpc.get_latest_block_number().await {
            Ok(block) => {
                if let Ok(mut guard) = self.cached_latest_block.write() {
                    *guard = Some((block, Instant::now()));
                }
                Some(block)
            }
            Err(e) => {
                error!("Failed to get latest block: {}", e);
                None
            }
        }
    }

    /// Get total network accounts from Etherscan
    pub async fn get_total_network_accounts(&self) -> Option<u64> {
        if let Ok(guard) = self.cached_network_accounts.read() {
            if let Some((value, timestamp)) = *guard {
                if timestamp.elapsed() < Self::CACHE_DURATION {
                    return Some(value);
                }
            }
        }
        None
    }

    async fn update_latest_block(&self) -> Result<()> {
        let block = self.rpc.get_latest_block_number().await?;
        if let Ok(mut guard) = self.cached_latest_block.write() {
            *guard = Some((block, Instant::now()));
        }
        Ok(())
    }

    async fn update_network_accounts(&self) -> Result<()> {
        let response = self
            .client
            .get(Self::ETHERSCAN_URL)
            .header(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            )
            .header("Accept-Language", "en-US,en;q=0.5")
            .header("Accept-Encoding", "identity")
            .header("Connection", "keep-alive")
            .header("Upgrade-Insecure-Requests", "1")
            .send()
            .await
            .context("Failed to fetch Etherscan page")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Etherscan returned status: {}",
                response.status()
            ));
        }

        let html = response
            .text()
            .await
            .context("Failed to read response text")?;

        // Find the line that starts with "var litChartData ="
        let mut chart_data_line = None;
        for line in html.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("var litChartData =") {
                chart_data_line = Some(trimmed);
                break;
            }
        }

        let chart_line = match chart_data_line {
            Some(line) => line,
            None => return Err(anyhow::anyhow!("litChartData line not found")),
        };

        // Extract the last y value from this line
        let y_re = Regex::new(r"y\s*:\s*(\d+)").context("Invalid y regex")?;
        let mut last_value = 0u64;

        for captures in y_re.captures_iter(chart_line) {
            if let Some(y_match) = captures.get(1) {
                if let Ok(value) = y_match.as_str().parse::<u64>() {
                    last_value = value;
                }
            }
        }

        if last_value > 0 {
            if let Ok(mut guard) = self.cached_network_accounts.write() {
                *guard = Some((last_value, Instant::now()));
            }
            info!("Updated network accounts: {}", last_value);
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Failed to extract network accounts from Etherscan"
            ))
        }
    }

    fn should_update_accounts(&self) -> bool {
        if let Ok(guard) = self.cached_network_accounts.read() {
            if let Some((_, timestamp)) = *guard {
                return timestamp.elapsed() >= Self::CACHE_DURATION;
            }
        }
        true
    }
}
