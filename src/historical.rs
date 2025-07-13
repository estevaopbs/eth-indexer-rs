use anyhow::Result;
use serde_json::{json, Value};
use std::sync::{Arc, RwLock};
use tracing::{error, info, warn};

use crate::config::AppConfig;
use crate::database::DatabaseService;

/// Service for managing historical transaction counts with BigQuery integration
pub struct HistoricalTransactionService {
    db: Arc<DatabaseService>,
    config: AppConfig,
    cached_historical_count: Arc<RwLock<Option<i64>>>,
}

impl HistoricalTransactionService {
    pub fn new(db: Arc<DatabaseService>, config: AppConfig) -> Self {
        Self {
            db,
            config,
            cached_historical_count: Arc::new(RwLock::new(None)),
        }
    }

    /// Initialize historical transaction count on application startup
    pub async fn initialize(&self, start_block: i64) -> Result<()> {
        info!("Initializing historical transaction service for block {}", start_block);

        // Check if we already have a cached value
        if let Ok(guard) = self.cached_historical_count.read() {
            if guard.is_some() {
                info!("Historical count already initialized");
                return Ok(());
            }
        }

        // Try to get from database cache first
        if let Some(cached_count) = self.db.get_cached_historical_count(start_block).await? {
            info!("Found cached historical count for block {}: {}", start_block, cached_count);
            if let Ok(mut guard) = self.cached_historical_count.write() {
                *guard = Some(cached_count);
            }
            return Ok(());
        }

        // Not in cache, try to estimate or fetch from BigQuery
        let estimated_count = self.estimate_historical_count(start_block).await?;
        
        // If estimation failed, try BigQuery
        let final_count = if estimated_count.is_none() {
            match self.fetch_from_bigquery(start_block).await {
                Ok(count) => {
                    // Save to cache for future use
                    self.db.cache_historical_count(start_block, count).await?;
                    Some(count)
                }
                Err(e) => {
                    warn!("BigQuery fetch failed: {}. Using fallback estimation.", e);
                    Some(self.fallback_estimation(start_block))
                }
            }
        } else {
            estimated_count
        };

        if let Some(count) = final_count {
            if let Ok(mut guard) = self.cached_historical_count.write() {
                *guard = Some(count);
            }
            info!("Historical transaction count initialized: {}", count);
        } else {
            error!("Failed to initialize historical transaction count");
        }

        Ok(())
    }

    /// Get the cached historical transaction count
    pub fn get_historical_count(&self) -> Option<i64> {
        self.cached_historical_count.read().ok().and_then(|guard| *guard)
    }

    /// Estimate historical count using cached data and regression analysis
    async fn estimate_historical_count(&self, target_block: i64) -> Result<Option<i64>> {
        let cached_data = self.db.get_all_cached_historical_counts().await?;
        
        if cached_data.is_empty() {
            info!("No cached data available for estimation");
            return Ok(None);
        }

        // Define estimation ranges (block ranges with estimated transactions per block)
        let estimation_ranges = self.get_estimation_ranges();
        
        // Find the closest range border to our target block
        let closest_border = self.find_closest_border(&estimation_ranges, target_block);
        
        // Check if we have cached data closer than the closest border
        if let Some(closest_cached) = self.find_closest_cached_point(&cached_data, target_block) {
            let distance_to_cached = (closest_cached.0 - target_block).abs();
            let distance_to_border = (closest_border - target_block).abs();
            
            if distance_to_cached <= distance_to_border {
                // Use cached data for estimation
                return Ok(Some(self.estimate_from_cached_data(&cached_data, target_block).await?));
            }
        }

        // Use range-based estimation
        Ok(Some(self.estimate_with_predefined_ranges(target_block)))
    }

    /// Estimate using cached data points (linear regression for 2+ points, linear interpolation for 1 point)
    async fn estimate_from_cached_data(&self, cached_data: &[(i64, i64)], target_block: i64) -> Result<i64> {
        if cached_data.len() >= 2 {
            // Use the two closest points for linear regression
            let two_closest = self.find_two_closest_cached_points(cached_data, target_block);
            let estimated = self.linear_interpolation(two_closest.0, two_closest.1, target_block);
            info!("Estimated {} transactions before block {} using linear regression", estimated, target_block);
            Ok(estimated)
        } else if cached_data.len() == 1 {
            // Use single point with range estimation
            let (cached_block, cached_count) = cached_data[0];
            let estimated = self.estimate_from_single_point(cached_block, cached_count, target_block);
            info!("Estimated {} transactions before block {} using single point estimation", estimated, target_block);
            Ok(estimated)
        } else {
            Err(anyhow::anyhow!("No cached data available"))
        }
    }

    /// Find the two closest cached points to the target block
    fn find_two_closest_cached_points(&self, cached_data: &[(i64, i64)], target_block: i64) -> ((i64, i64), (i64, i64)) {
        let mut distances: Vec<_> = cached_data.iter()
            .map(|&point| (point, (point.0 - target_block).abs()))
            .collect();
        
        distances.sort_by_key(|&(_, dist)| dist);
        
        (distances[0].0, distances[1].0)
    }

    /// Perform linear interpolation/extrapolation between two points
    fn linear_interpolation(&self, point1: (i64, i64), point2: (i64, i64), target_block: i64) -> i64 {
        let (x1, y1) = (point1.0 as f64, point1.1 as f64);
        let (x2, y2) = (point2.0 as f64, point2.1 as f64);
        let x = target_block as f64;
        
        // Linear equation: y = y1 + (y2 - y1) * (x - x1) / (x2 - x1)
        let estimated = y1 + (y2 - y1) * (x - x1) / (x2 - x1);
        
        estimated.max(0.0) as i64
    }

    /// Estimate from a single cached point using range-based estimation
    fn estimate_from_single_point(&self, cached_block: i64, cached_count: i64, target_block: i64) -> i64 {
        let block_diff = target_block - cached_block;
        let range = self.find_range_for_block(target_block);
        let estimated_txs_per_block = range.1;
        
        let adjustment = block_diff * estimated_txs_per_block;
        (cached_count + adjustment).max(0)
    }

    /// Estimate using predefined ranges when no suitable cached data is available
    fn estimate_with_predefined_ranges(&self, target_block: i64) -> i64 {
        let ranges = self.get_estimation_ranges();
        
        // Find the appropriate range for the target block
        let range = self.find_range_for_block(target_block);
        let (range_start, txs_per_block) = range;
        
        // Calculate cumulative transactions up to the range start
        let mut cumulative = 0i64;
        for (start, end, rate) in &ranges {
            if *end < range_start {
                cumulative += (*end - *start) * rate;
            } else if *start < range_start {
                cumulative += (range_start - *start) * rate;
                break;
            } else {
                break;
            }
        }
        
        // Add transactions from range start to target block
        cumulative += (target_block - range_start) * txs_per_block;
        
        info!("Estimated {} transactions before block {} using predefined ranges", cumulative, target_block);
        cumulative.max(0)
    }

    /// Get predefined estimation ranges (start_block, end_block, transactions_per_block)
    fn get_estimation_ranges(&self) -> Vec<(i64, i64, i64)> {
        // Updated with more realistic data based on BigQuery results
        // We know block 15,000,000 has 1,613,669,773 total transactions
        vec![
            (0, 1_000_000, 10),              // Early Ethereum (2015-2016): ~10 tx/block (~10M total)
            (1_000_000, 4_000_000, 20),      // Growing adoption (2016-2017): ~20 tx/block (~70M total)  
            (4_000_000, 8_000_000, 35),      // ICO boom (2017-2019): ~35 tx/block (~210M total)
            (8_000_000, 12_000_000, 60),     // DeFi emergence (2019-2021): ~60 tx/block (~450M total)
            (12_000_000, 15_000_000, 120),   // DeFi/NFT boom (2021-2022): ~120 tx/block (reaches ~1.6B)
            (15_000_000, 17_000_000, 130),   // Current high activity: ~130 tx/block
            (17_000_000, 20_000_000, 140),   // Future growth: ~140 tx/block
            (20_000_000, i64::MAX, 150),     // Long-term: ~150 tx/block
        ]
    }

    /// Find the range that contains the target block
    fn find_range_for_block(&self, target_block: i64) -> (i64, i64) {
        let ranges = self.get_estimation_ranges();
        
        for (start, end, rate) in ranges {
            if target_block >= start && target_block < end {
                return (start, rate);
            }
        }
        
        // Default to the last range
        (20_000_000, 200)
    }

    /// Find the closest border among all estimation ranges
    fn find_closest_border(&self, ranges: &[(i64, i64, i64)], target_block: i64) -> i64 {
        let mut closest_distance = i64::MAX;
        let mut closest_border = 0;
        
        for (start, end, _) in ranges {
            let start_distance = (*start - target_block).abs();
            let end_distance = (*end - target_block).abs();
            
            if start_distance < closest_distance {
                closest_distance = start_distance;
                closest_border = *start;
            }
            
            if end_distance < closest_distance {
                closest_distance = end_distance;
                closest_border = *end;
            }
        }
        
        closest_border
    }

    /// Find the closest cached point to the target block
    fn find_closest_cached_point(&self, cached_data: &[(i64, i64)], target_block: i64) -> Option<(i64, i64)> {
        cached_data.iter()
            .min_by_key(|(block, _)| (*block - target_block).abs())
            .copied()
    }

    /// Fallback estimation when all else fails
    fn fallback_estimation(&self, target_block: i64) -> i64 {
        // Use realistic estimates based on known data points
        // We know block 15,000,000 = 1,613,669,773 transactions
        let estimated = match target_block {
            0..=1_000_000 => 10_000_000,          // ~10M transactions by block 1M
            1_000_001..=4_000_000 => 70_000_000,  // ~70M transactions by block 4M
            4_000_001..=8_000_000 => 210_000_000, // ~210M transactions by block 8M  
            8_000_001..=12_000_000 => 450_000_000, // ~450M transactions by block 12M
            12_000_001..=15_000_000 => {
                // Linear interpolation to known value at block 15M
                let base = 450_000_000;
                let diff = target_block - 12_000_000;
                let rate = (1_613_669_773 - base) / (15_000_000 - 12_000_000);
                base + (diff * rate)
            },
            15_000_001..=17_000_000 => {
                // Estimate from known block 15M value
                let base = 1_613_669_773;
                let diff = target_block - 15_000_000;
                base + (diff * 130) // ~130 tx/block
            },
            17_000_001..=20_000_000 => {
                let base = 1_873_669_773; // Estimated for block 17M
                let diff = target_block - 17_000_000;
                base + (diff * 140) // ~140 tx/block
            },
            _ => {
                let base = 2_293_669_773; // Estimated for block 20M
                let diff = target_block - 20_000_000;
                base + (diff * 150) // ~150 tx/block
            }
        };
        
        info!("Using fallback estimation: {} transactions before block {}", estimated, target_block);
        estimated.max(0)
    }

    /// Fetch historical transaction count from BigQuery
    async fn fetch_from_bigquery(&self, target_block: i64) -> Result<i64> {
        info!(
            "Fetching historical transaction count for block {} from BigQuery",
            target_block
        );

        // Verificar se temos service account path configurado
        let service_account_path = match &self.config.bigquery_service_account_path {
            Some(path) => path,
            None => {
                warn!("BIGQUERY_SERVICE_ACCOUNT_PATH not configured");
                return Err(anyhow::anyhow!("BigQuery service account path not configured"));
            }
        };

        // Carregar service account
        let custom_service_account =
            match gcp_auth::CustomServiceAccount::from_file(service_account_path) {
                Ok(account) => account,
                Err(e) => {
                    error!(
                        "Failed to load service account from file {}: {}",
                        service_account_path, e
                    );
                    return Err(anyhow::anyhow!("Failed to load service account: {}", e));
                }
            };

        let auth_manager = gcp_auth::AuthenticationManager::from(custom_service_account);

        // Obter project_id do service account
        let project_id = match auth_manager.project_id().await {
            Ok(id) => id,
            Err(e) => {
                error!("Failed to get project ID: {}", e);
                return Err(anyhow::anyhow!("Failed to get project ID: {}", e));
            }
        };

        // Obter token de acesso
        let scopes = &["https://www.googleapis.com/auth/bigquery.readonly"];
        let token = match auth_manager.get_token(scopes).await {
            Ok(token) => token,
            Err(e) => {
                error!("Failed to get GCP access token: {}", e);
                return Err(anyhow::anyhow!("Failed to get GCP access token: {}", e));
            }
        };

        // Execute dynamic query on public BigQuery Ethereum dataset
        let client = reqwest::Client::new();
        let query_url = format!(
            "https://bigquery.googleapis.com/bigquery/v2/projects/{}/queries",
            project_id
        );

        // Monta query dinamicamente para contar transações até o bloco especificado
        let sql_query = format!(
            "SELECT COUNT(*) as total_transactions FROM `bigquery-public-data.crypto_ethereum.transactions` WHERE block_number <= {}",
            target_block
        );

        let query_body = json!({
            "query": sql_query,
            "useLegacySql": false,
            "maxResults": 1,
            "timeoutMs": 30000
        });

        let response = match client
            .post(&query_url)
            .bearer_auth(token.as_str())
            .json(&query_body)
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                error!("Failed to execute BigQuery query: {}", e);
                return Err(anyhow::anyhow!("Failed to execute BigQuery query: {}", e));
            }
        };

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            warn!("BigQuery API error: {}", error_text);
            return Err(anyhow::anyhow!("BigQuery API error: {}", error_text));
        }

        let result: Value = match response.json().await {
            Ok(result) => result,
            Err(e) => {
                error!("Failed to parse BigQuery response: {}", e);
                return Err(anyhow::anyhow!("Failed to parse BigQuery response: {}", e));
            }
        };

        // Extract the total transaction count from the response
        if let Some(rows) = result["rows"].as_array() {
            if let Some(first_row) = rows.first() {
                if let Some(fields) = first_row["f"].as_array() {
                    if let Some(count_field) = fields.first() {
                        if let Some(count_str) = count_field["v"].as_str() {
                            match count_str.parse::<i64>() {
                                Ok(count) => {
                                    info!(
                                        "BigQuery returned {} total transactions up to block {}",
                                        count, target_block
                                    );
                                    return Ok(count);
                                }
                                Err(e) => {
                                    error!(
                                        "Failed to parse transaction count from BigQuery: {}",
                                        e
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        Err(anyhow::anyhow!("Unexpected BigQuery response format"))
    }

}
