use crate::{
    database::{DatabaseService, Token, TokenBalance},
    rpc::RpcClient,
};
use anyhow::{Context, Result};
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use tokio::time::{sleep, Duration};

/// Service for managing token information and balances
pub struct TokenService {
    db: Arc<DatabaseService>,
    rpc: Arc<RpcClient>,
}

impl TokenService {
    /// Create a new token service
    pub fn new(db: Arc<DatabaseService>, rpc: Arc<RpcClient>) -> Self {
        Self { db, rpc }
    }

    /// Discover token information from contract address
    pub async fn discover_token(&self, token_address: &str, block_number: i64) -> Result<Token> {
        // Check if token already exists in database
        if let Some(existing_token) = self.db.get_token_by_address(token_address).await? {
            return Ok(existing_token);
        }

        info!("Discovering new token: {}", token_address);

        // Get token metadata from contract
        let name = self.rpc.get_token_name(token_address).await.unwrap_or(None);
        let symbol = self.rpc.get_token_symbol(token_address).await.unwrap_or(None);
        let decimals = self.rpc.get_token_decimals(token_address).await.unwrap_or(None);

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

        info!(
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
        info!(
            "Updating token balance for {} holding {} at block {}",
            account_address, token_address, block_number
        );

        // Get current balance from RPC
        match self
            .rpc
            .get_token_balance(token_address, account_address, Some(block_number as u64))
            .await
        {
            Ok(balance) => {
                info!(
                    "Retrieved balance {} for {} holding {}",
                    balance, account_address, token_address
                );

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

                match self.db.upsert_token_balance(&token_balance).await {
                    Ok(_) => {
                        info!(
                            "Successfully updated token balance: {} {} for {}",
                            balance, token_address, account_address
                        );
                    }
                    Err(e) => {
                        error!(
                            "Failed to upsert token balance for {} holding {}: {}",
                            account_address, token_address, e
                        );
                        return Err(e);
                    }
                }
            }
            Err(e) => {
                warn!(
                    "Failed to get token balance for {} holding {}: {}",
                    account_address, token_address, e
                );
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
        info!("Starting balance updates for {} transfers at block {}", transfers.len(), block_number);
        let mut unique_updates = std::collections::HashSet::new();

        // Collect unique (account, token) pairs
        for (token_address, from_address, to_address) in transfers {
            unique_updates.insert((from_address.clone(), token_address.clone()));
            unique_updates.insert((to_address.clone(), token_address.clone()));
        }

        info!("Collected {} unique (account, token) pairs to update", unique_updates.len());

        // Update balances for each unique pair
        for (i, (account_address, token_address)) in unique_updates.iter().enumerate() {
            // Skip null address (0x0000...)
            if account_address.starts_with("0x0000000000000000000000000000000000000000") {
                info!("Skipping null address: {}", account_address);
                continue;
            }

            info!("Updating balance {}/{}: {} holding {}", 
                  i + 1, unique_updates.len(), account_address, token_address);

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
            sleep(Duration::from_millis(10)).await;
        }

        info!("Completed balance updates for block {}", block_number);
        Ok(())
    }

    /// Refresh stale token balances
    pub async fn refresh_stale_balances(&self, current_block: i64, max_age_blocks: i64) -> Result<()> {
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
            sleep(Duration::from_millis(50)).await;
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
