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
        info!(
            "Initializing historical transaction service for block {}",
            start_block
        );

        // Check if we already have a cached value
        if let Ok(guard) = self.cached_historical_count.read() {
            if guard.is_some() {
                info!("Historical count already initialized");
                return Ok(());
            }
        }

        // Try to get from database cache first
        if let Some(cached_count) = self.db.get_cached_historical_count(start_block).await? {
            info!(
                "Found cached historical count for block {}: {}",
                start_block, cached_count
            );
            if let Ok(mut guard) = self.cached_historical_count.write() {
                *guard = Some(cached_count);
            }
            return Ok(());
        }

        // Try to fetch from BigQuery
        match self.fetch_from_bigquery(start_block).await {
            Ok(count) => {
                // Save to cache for future use
                self.db.cache_historical_count(start_block, count).await?;
                if let Ok(mut guard) = self.cached_historical_count.write() {
                    *guard = Some(count);
                }
                info!(
                    "Historical transaction count initialized from BigQuery: {}",
                    count
                );
            }
            Err(e) => {
                warn!(
                    "BigQuery fetch failed: {}. Historical count will be unavailable.",
                    e
                );
            }
        }

        Ok(())
    }

    /// Get the cached historical transaction count
    pub fn get_historical_count(&self) -> Option<i64> {
        self.cached_historical_count
            .read()
            .ok()
            .and_then(|guard| *guard)
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
                return Err(anyhow::anyhow!(
                    "BigQuery service account path not configured"
                ));
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
