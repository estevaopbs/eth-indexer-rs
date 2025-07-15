use crate::{
    config::AppConfig,
    database::{DatabaseService, Token, TokenBalance},
    rpc::RpcClient,
};
use anyhow::Result;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn};

/// Service for managing token information and balances
pub struct TokenService {
    db: Arc<DatabaseService>,
    rpc: Arc<RpcClient>,
    config: AppConfig,
}

impl TokenService {
    /// Create a new token service
    pub fn new(db: Arc<DatabaseService>, rpc: Arc<RpcClient>, config: AppConfig) -> Self {
        Self { db, rpc, config }
    }

    /// Discover token information from contract address
    pub async fn discover_token(&self, token_address: &str, block_number: i64) -> Result<Token> {
        // Check if token already exists in database
        if let Some(existing_token) = self.db.get_token_by_address(token_address).await? {
            return Ok(existing_token);
        }

        // First verify this is actually a contract and supports basic ERC-20 methods
        // Try to get token name/symbol as a basic validation
        let name = self.rpc.get_token_name(token_address).await.unwrap_or(None);
        let symbol = self
            .rpc
            .get_token_symbol(token_address)
            .await
            .unwrap_or(None);
        let decimals = self
            .rpc
            .get_token_decimals(token_address)
            .await
            .unwrap_or(None);

        // If we can't get any token metadata, it's likely not a valid ERC-20 contract
        if name.is_none() && symbol.is_none() && decimals.is_none() {
            return Err(anyhow::anyhow!(
                "Token address {} does not appear to be a valid ERC-20 contract (no name, symbol, or decimals)",
                token_address
            ));
        }

        let token = Token {
            address: token_address.to_string(),
            name,
            symbol,
            decimals,
            token_type: "ERC20".to_string(), // Default to ERC20
            first_seen_block: block_number,
            last_seen_block: block_number,
            total_transfers: 1,
            created_at: None,
            updated_at: None,
        };

        // Save to database
        self.db.upsert_token(&token).await?;

        debug!(
            "Discovered token: {} ({}) at {}",
            token.name.as_deref().unwrap_or("Unknown"),
            token.symbol.as_deref().unwrap_or("?"),
            token_address
        );

        Ok(token)
    }

    /// Update token balance for an account
    pub async fn update_token_balance(
        &self,
        account_address: &str,
        token_address: &str,
        block_number: i64,
    ) -> Result<()> {
        // Get current balance from RPC
        match self
            .rpc
            .get_token_balance(token_address, account_address, Some(block_number as u64))
            .await
        {
            Ok(balance) => {
                let token_balance = TokenBalance {
                    id: None,
                    account_address: account_address.to_string(),
                    token_address: token_address.to_string(),
                    balance: balance.clone(),
                    block_number,
                    last_updated_block: block_number,
                    created_at: None,
                    updated_at: None,
                };

                self.db.upsert_token_balance(&token_balance).await?;
            }
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("not a contract") {
                    debug!(
                        "Skipping token balance update for {} holding {} - address is not a contract",
                        account_address, token_address
                    );
                    // Mark this token as invalid to avoid future attempts
                    // You could implement a blacklist mechanism here
                } else if error_msg.contains("does not implement ERC-20") {
                    debug!(
                        "Skipping token balance update for {} holding {} - contract does not implement ERC-20 balanceOf",
                        account_address, token_address
                    );
                } else {
                    warn!(
                        "Failed to get token balance for {} holding {}: {}",
                        account_address, token_address, e
                    );
                }
            }
        }

        Ok(())
    }

    /// Update token balances for all accounts affected by token transfers in a block
    pub async fn update_balances_for_transfers(
        &self,
        transfers: &[(String, String, String)], // (token_address, from_address, to_address)
        block_number: i64,
    ) -> Result<()> {
        info!(
            "Starting balance updates for {} transfers at block {}",
            transfers.len(),
            block_number
        );
        let mut unique_updates = std::collections::HashSet::new();

        // Collect unique (account, token) pairs
        for (token_address, from_address, to_address) in transfers {
            unique_updates.insert((from_address.clone(), token_address.clone()));
            unique_updates.insert((to_address.clone(), token_address.clone()));
        }

        debug!(
            "Collected {} unique (account, token) pairs to update",
            unique_updates.len()
        );

        // Update balances for each unique pair
        for (_i, (account_address, token_address)) in unique_updates.iter().enumerate() {
            // Skip null address (0x0000...)
            if account_address.starts_with("0x0000000000000000000000000000000000000000") {
                debug!("Skipping null address: {}", account_address);
                continue;
            }

            if let Err(e) = self
                .update_token_balance(account_address, token_address, block_number)
                .await
            {
                error!(
                    "Failed to update token balance for {} holding {}: {}",
                    account_address, token_address, e
                );
            }

            // Small delay to avoid overwhelming the RPC
            sleep(Duration::from_millis(
                self.config.token_balance_update_interval_ms,
            ))
            .await;
        }

        info!("Completed balance updates for block {}", block_number);
        Ok(())
    }

    /// Refresh stale token balances
    pub async fn refresh_stale_balances(
        &self,
        current_block: i64,
        max_age_blocks: i64,
    ) -> Result<()> {
        let min_block = current_block - max_age_blocks;
        let stale_balances = self.db.get_stale_token_balances(min_block, 100).await?;

        info!(
            "Found {} stale token balances to refresh",
            stale_balances.len()
        );

        for balance in stale_balances {
            if let Err(e) = self
                .update_token_balance(
                    &balance.account_address,
                    &balance.token_address,
                    current_block,
                )
                .await
            {
                error!(
                    "Failed to refresh token balance for {} holding {}: {}",
                    balance.account_address, balance.token_address, e
                );
            }

            // Small delay to avoid overwhelming the RPC
            sleep(Duration::from_millis(self.config.token_refresh_interval_ms)).await;
        }

        Ok(())
    }

    /// Get token with balance information for an account
    pub async fn get_account_token_info(
        &self,
        account_address: &str,
    ) -> Result<Vec<(Token, TokenBalance)>> {
        let balances = self.db.get_account_token_balances(account_address).await?;
        let mut result = Vec::new();

        for balance in balances {
            if let Some(token) = self.db.get_token_by_address(&balance.token_address).await? {
                result.push((token, balance));
            }
        }

        Ok(result)
    }

    /// Start background service to periodically refresh token balances
    pub async fn start_background_refresh(
        &self,
        refresh_interval: Duration,
        max_age_blocks: i64,
    ) -> Result<()> {
        info!("Starting token balance refresh service");

        loop {
            // Get current block number
            match self.rpc.get_latest_block_number().await {
                Ok(current_block) => {
                    if let Err(e) = self
                        .refresh_stale_balances(current_block as i64, max_age_blocks)
                        .await
                    {
                        error!("Failed to refresh stale token balances: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to get current block number: {}", e);
                }
            }

            sleep(refresh_interval).await;
        }
    }
}
