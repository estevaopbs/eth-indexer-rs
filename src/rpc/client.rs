use crate::config::AppConfig;
use anyhow::{Context, Result};
use ethers::{
    core::types::{
        Block as EthBlock, BlockNumber, Transaction as EthTransaction, TransactionReceipt, H256,
        U64,
    },
    providers::{Http, Middleware, Provider},
};
use serde_json::{json, Value};
use std::str::FromStr;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Client for interacting with Ethereum RPC
pub struct RpcClient {
    provider: Arc<Provider<Http>>,
    config: AppConfig,
}

impl RpcClient {
    /// Create a new RPC client
    pub fn new(rpc_url: &str, config: AppConfig) -> Result<Self> {
        let provider = Provider::<Http>::try_from(rpc_url)
            .context(format!("Failed to connect to RPC URL: {}", rpc_url))?;

        Ok(Self {
            provider: Arc::new(provider),
            config,
        })
    }

    /// Get the latest block number
    pub async fn get_latest_block_number(&self) -> Result<u64> {
        let block_number = self
            .provider
            .get_block_number()
            .await
            .context("Failed to get latest block number")?;

        Ok(block_number.as_u64())
    }

    /// Get block by number
    pub async fn get_block_by_number(
        &self,
        number: u64,
    ) -> Result<Option<EthBlock<EthTransaction>>> {
        let block_number = BlockNumber::Number(number.into());
        let block = self
            .provider
            .get_block_with_txs(block_number)
            .await
            .context(format!("Failed to get block by number: {}", number))?;

        Ok(block)
    }

    /// Get block by hash
    pub async fn get_block_by_hash(&self, hash: &str) -> Result<Option<EthBlock<EthTransaction>>> {
        let hash = H256::from_str(hash).context(format!("Invalid block hash: {}", hash))?;

        let block = self
            .provider
            .get_block_with_txs(hash)
            .await
            .context(format!("Failed to get block by hash: {}", hash))?;

        Ok(block)
    }

    /// Get transaction receipt
    pub async fn get_transaction_receipt(
        &self,
        tx_hash: &str,
    ) -> Result<Option<TransactionReceipt>> {
        let hash =
            H256::from_str(tx_hash).context(format!("Invalid transaction hash: {}", tx_hash))?;

        let receipt = self
            .provider
            .get_transaction_receipt(hash)
            .await
            .context(format!("Failed to get transaction receipt: {}", tx_hash))?;

        Ok(receipt)
    }

    /// Get account balance
    pub async fn get_balance(&self, address: &str, block_number: Option<u64>) -> Result<String> {
        let address = address
            .parse::<ethers::core::types::H160>()
            .context(format!("Invalid Ethereum address: {}", address))?;

        let balance = match block_number {
            Some(num) => {
                self.provider
                    .get_balance(
                        address,
                        Some(ethers::core::types::BlockId::Number(BlockNumber::Number(
                            U64::from(num),
                        ))),
                    )
                    .await
            }
            None => self.provider.get_balance(address, None).await,
        }
        .context(format!("Failed to get balance for address: {}", address))?;

        Ok(balance.to_string())
    }

    /// Check connection to RPC
    pub async fn check_connection(&self) -> Result<bool> {
        match self.provider.get_block_number().await {
            Ok(_) => {
                debug!("Successfully connected to Ethereum RPC");
                Ok(true)
            }
            Err(e) => {
                error!("Failed to connect to Ethereum RPC: {}", e);
                Ok(false)
            }
        }
    }

    /// Get historical transaction count from BigQuery using dynamic query
    /// This queries the public Ethereum dataset to count total transactions up to a specific block
    pub async fn get_historical_transaction_count_from_bigquery(
        &self,
        target_block: u64,
    ) -> Result<u64> {
        info!(
            "Fetching historical transaction count for block {} from BigQuery",
            target_block
        );

        // Verificar se temos service account path configurado
        let service_account_path = match &self.config.bigquery_service_account_path {
            Some(path) => path,
            None => {
                warn!("BIGQUERY_SERVICE_ACCOUNT_PATH not configured");
                return self.get_fallback_estimate(target_block);
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
                    return self.get_fallback_estimate(target_block);
                }
            };

        let auth_manager = gcp_auth::AuthenticationManager::from(custom_service_account);

        // Obter project_id do service account
        let project_id = match auth_manager.project_id().await {
            Ok(id) => id,
            Err(e) => {
                error!("Failed to get project ID: {}", e);
                return self.get_fallback_estimate(target_block);
            }
        };

        // Obter token de acesso
        let scopes = &["https://www.googleapis.com/auth/bigquery.readonly"];
        let token = match auth_manager.get_token(scopes).await {
            Ok(token) => token,
            Err(e) => {
                error!("Failed to get GCP access token: {}", e);
                return self.get_fallback_estimate(target_block);
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
                return self.get_fallback_estimate(target_block);
            }
        };

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            warn!("BigQuery API error: {}", error_text);
            return self.get_fallback_estimate(target_block);
        }

        let result: Value = match response.json().await {
            Ok(result) => result,
            Err(e) => {
                error!("Failed to parse BigQuery response: {}", e);
                return self.get_fallback_estimate(target_block);
            }
        };

        // Extract the total transaction count from the response
        if let Some(rows) = result["rows"].as_array() {
            if let Some(first_row) = rows.first() {
                if let Some(fields) = first_row["f"].as_array() {
                    if let Some(count_field) = fields.first() {
                        if let Some(count_str) = count_field["v"].as_str() {
                            match count_str.parse::<u64>() {
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

        warn!("Unexpected BigQuery response format, using fallback");
        self.get_fallback_estimate(target_block)
    }

    /// Get fallback estimate when BigQuery is not available
    fn get_fallback_estimate(&self, target_block: u64) -> Result<u64> {
        let estimated = match target_block {
            0..=1000000 => 100_000,
            1000001..=4000000 => 50_000_000,
            4000001..=8000000 => 350_000_000,
            8000001..=12000000 => 950_000_000,
            12000001..=15000000 => 1_500_000_000,
            15000001..=17000000 => 1_800_000_000,
            17000001..=20000000 => 2_200_000_000,
            _ => 2_500_000_000,
        };

        warn!(
            "Using fallback estimation for block {}: {}",
            target_block, estimated
        );
        Ok(estimated)
    }
}
