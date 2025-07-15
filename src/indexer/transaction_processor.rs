use crate::{
    config::AppConfig,
    database::{Account, DatabaseService, Log, TokenTransfer, Transaction},
    rpc::RpcClient,
    token_service::TokenService,
};
use anyhow::{Context, Result};
use ethers::core::types::{Log as EthLog, Transaction as EthTransaction, TransactionReceipt};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use tracing::{debug, error, warn};

/// Processor for handling transaction data
#[derive(Clone)]
pub struct TransactionProcessor {
    db: Arc<DatabaseService>,
    rpc: Arc<RpcClient>,
    config: AppConfig,
    token_service: Option<Arc<TokenService>>,
    account_cache: Arc<RwLock<HashMap<String, Option<Account>>>>,
}

impl TransactionProcessor {
    /// Create a new transaction processor
    pub fn new(db: Arc<DatabaseService>, rpc: Arc<RpcClient>, config: AppConfig) -> Self {
        Self {
            db,
            rpc,
            config,
            token_service: None,
            account_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new transaction processor with token service
    pub fn with_token_service(
        db: Arc<DatabaseService>,
        rpc: Arc<RpcClient>,
        config: AppConfig,
        token_service: Arc<TokenService>,
    ) -> Self {
        Self {
            db,
            rpc,
            config,
            token_service: Some(token_service),
            account_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get transaction receipts in batch for better performance
    pub async fn get_transaction_receipts_batch(
        &self,
        tx_hashes: &[String],
    ) -> Result<Vec<Option<TransactionReceipt>>> {
        use futures::future;

        // Create semaphore to limit concurrent requests using config parameter
        let semaphore = Arc::new(tokio::sync::Semaphore::new(
            self.config.max_concurrent_tx_receipts,
        ));

        // Create tasks for all receipt requests
        let tasks: Vec<_> = tx_hashes
            .iter()
            .map(|hash| {
                let rpc = self.rpc.clone();
                let hash = hash.clone();
                let semaphore = semaphore.clone();

                async move {
                    let _permit = semaphore.acquire().await?;
                    rpc.get_transaction_receipt(&hash).await
                }
            })
            .collect();

        // Execute all requests concurrently
        let results = future::try_join_all(tasks).await?;
        Ok(results)
    }

    /// Collect data for multiple transactions efficiently (block-level batch processing)
    pub async fn collect_block_transaction_data(
        &self,
        transactions_with_receipts: &[(EthTransaction, TransactionReceipt)],
    ) -> Result<(Vec<Transaction>, Vec<Log>, Vec<TokenTransfer>, Vec<Account>)> {
        let mut all_transactions = Vec::new();
        let mut all_logs = Vec::new();
        let mut all_token_transfers = Vec::new();
        let mut unique_addresses = std::collections::HashSet::new();

        // First pass: collect all data without account processing
        for (eth_tx, receipt) in transactions_with_receipts {
            let tx = self.convert_transaction(eth_tx, receipt)?;

            // Collect transaction logs
            for eth_log in &receipt.logs {
                let log = self.convert_log(&tx, eth_log)?;
                all_logs.push(log);

                // Check if it's a token transfer
                if eth_log.topics.len() >= 3
                    && format!("0x{}", hex::encode(eth_log.topics[0].as_bytes()))
                        == "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
                {
                    if let Ok(transfer) = self.process_erc20_transfer(&tx, eth_log).await {
                        all_token_transfers.push(transfer);
                    }
                }
            }

            // Collect unique addresses
            let from_address = format!("{:#x}", eth_tx.from);
            unique_addresses.insert(from_address);

            if let Some(to_addr) = eth_tx.to {
                let to_address = format!("{:#x}", to_addr);
                unique_addresses.insert(to_address);
            }

            all_transactions.push(tx);
        }

        // Second pass: batch process accounts for unique addresses only
        let unique_addresses: Vec<String> = unique_addresses.into_iter().collect();

        // Use the first transaction's block number as reference
        let block_number = if let Some((first_tx, _)) = transactions_with_receipts.first() {
            first_tx
                .block_number
                .map(|n| n.as_u64() as i64)
                .unwrap_or(0)
        } else {
            0
        };

        // Use optimized batch processing for accounts
        let all_accounts = self
            .prepare_accounts_batch(&unique_addresses, block_number)
            .await?;
        debug!(
            "Prepared {} accounts for batch insertion",
            all_accounts.len()
        );

        Ok((
            all_transactions,
            all_logs,
            all_token_transfers,
            all_accounts,
        ))
    }

    /// Process ERC20 transfer from log
    async fn process_erc20_transfer(
        &self,
        tx: &Transaction,
        eth_log: &EthLog,
    ) -> Result<TokenTransfer> {
        // Extract from and to addresses from topics
        let from_address = if eth_log.topics.len() > 1 {
            format!("0x{}", hex::encode(&eth_log.topics[1].as_bytes()[12..]))
        } else {
            "0x0000000000000000000000000000000000000000".to_string()
        };

        let to_address = if eth_log.topics.len() > 2 {
            format!("0x{}", hex::encode(&eth_log.topics[2].as_bytes()[12..]))
        } else {
            "0x0000000000000000000000000000000000000000".to_string()
        };

        // Extract amount from data
        let amount = if eth_log.data.0.len() >= 32 {
            let mut amount_bytes = [0u8; 32];
            let data_len = eth_log.data.0.len();
            if data_len >= 32 {
                amount_bytes.copy_from_slice(&eth_log.data.0[data_len - 32..]);
            } else {
                amount_bytes[32 - data_len..].copy_from_slice(&eth_log.data.0);
            }
            ethers::types::U256::from_big_endian(&amount_bytes).to_string()
        } else {
            "0".to_string()
        };

        let transfer = TokenTransfer {
            id: None,
            transaction_hash: tx.hash.clone(),
            token_address: format!("{:#x}", eth_log.address),
            from_address,
            to_address,
            amount,
            block_number: tx.block_number,
            token_type: Some("ERC20".to_string()),
            token_id: None,
        };

        Ok(transfer)
    }

    /// Convert Ethereum transaction to our Transaction model
    fn convert_transaction(
        &self,
        eth_tx: &EthTransaction,
        receipt: &TransactionReceipt,
    ) -> Result<Transaction> {
        let tx = Transaction {
            hash: format!("{:#x}", eth_tx.hash),
            block_number: eth_tx
                .block_number
                .context("Block number missing")?
                .as_u64() as i64,
            from_address: format!("{:#x}", eth_tx.from),
            to_address: eth_tx.to.map(|addr| format!("{:#x}", addr)),
            value: eth_tx.value.to_string(),
            gas_used: receipt.gas_used.unwrap_or_default().as_u64() as i64,
            gas_price: eth_tx.gas_price.unwrap_or_default().to_string(),
            status: receipt
                .status
                .context("Transaction status missing")?
                .as_u64() as i64,
            transaction_index: receipt.transaction_index.as_u64() as i64,
        };

        Ok(tx)
    }

    /// Convert Ethereum log to our Log model
    fn convert_log(&self, tx: &Transaction, eth_log: &EthLog) -> Result<Log> {
        let log = Log {
            id: None,
            transaction_hash: tx.hash.clone(),
            block_number: tx.block_number,
            address: format!("{:#x}", eth_log.address),
            topic0: if eth_log.topics.len() > 0 {
                Some(format!("0x{}", hex::encode(eth_log.topics[0].as_bytes())))
            } else {
                None
            },
            topic1: if eth_log.topics.len() > 1 {
                Some(format!("0x{}", hex::encode(eth_log.topics[1].as_bytes())))
            } else {
                None
            },
            topic2: if eth_log.topics.len() > 2 {
                Some(format!("0x{}", hex::encode(eth_log.topics[2].as_bytes())))
            } else {
                None
            },
            topic3: if eth_log.topics.len() > 3 {
                Some(format!("0x{}", hex::encode(eth_log.topics[3].as_bytes())))
            } else {
                None
            },
            data: if eth_log.data.0.len() > 0 {
                Some(format!("0x{}", hex::encode(&eth_log.data.0)))
            } else {
                None
            },
            log_index: eth_log.log_index.unwrap_or_default().as_u64() as i64,
        };

        Ok(log)
    }

    /// Prepare accounts for batch insertion with optimized balance fetching
    pub async fn prepare_accounts_batch(
        &self,
        addresses: &[String],
        block_number: i64,
    ) -> Result<Vec<Account>> {
        if addresses.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_accounts = Vec::new();
        let batch_size = self.config.rpc_batch_size;

        for chunk in addresses.chunks(batch_size) {
            let mut batch_accounts = Vec::new();

            // Create semaphore to limit concurrent balance fetches
            let semaphore = Arc::new(tokio::sync::Semaphore::new(
                self.config.max_concurrent_balance_fetches,
            ));

            // Fetch balances concurrently within the batch
            let balance_tasks: Vec<_> = chunk
                .iter()
                .map(|address| {
                    let rpc = self.rpc.clone();
                    let address = address.clone();
                    let semaphore = semaphore.clone();

                    async move {
                        let _permit = semaphore.acquire().await?;
                        let balance =
                            match rpc.get_balance(&address, Some(block_number as u64)).await {
                                Ok(bal) => bal.to_string(),
                                Err(e) => {
                                    debug!("Failed to get balance for {}: {}, using 0", address, e);
                                    "0".to_string()
                                }
                            };
                        Ok::<(String, String), anyhow::Error>((address, balance))
                    }
                })
                .collect();

            // Execute balance fetches concurrently
            let balance_results = futures::future::try_join_all(balance_tasks).await?;

            // Process each account with its balance
            for (address, balance) in balance_results {
                // Get existing account or create new one using cache
                let existing_account = self.get_account_cached(&address).await?;

                let account = if let Some(mut existing) = existing_account {
                    existing.balance = balance;
                    existing.transaction_count += 1;
                    existing.last_seen_block = block_number;
                    existing
                } else {
                    let new_account = Account {
                        address: address.clone(),
                        balance,
                        transaction_count: 1,
                        first_seen_block: block_number,
                        last_seen_block: block_number,
                    };
                    new_account
                };

                batch_accounts.push(account);
            }

            all_accounts.extend(batch_accounts);

            // Small delay between batches to avoid overwhelming RPC
            if addresses.len() > batch_size {
                tokio::time::sleep(tokio::time::Duration::from_millis(
                    self.config.eth_rpc_min_interval_ms,
                ))
                .await;
            }
        }

        Ok(all_accounts)
    }

    /// Get account with caching to reduce database queries
    async fn get_account_cached(&self, address: &str) -> Result<Option<Account>> {
        // Check cache first
        {
            let cache = self.account_cache.read().await;
            if let Some(cached_account) = cache.get(address) {
                return Ok(cached_account.clone());
            }
        }

        // If not in cache, fetch from database
        let account = self.db.get_account_by_address(address).await?;

        // Store in cache
        {
            let mut cache = self.account_cache.write().await;
            cache.insert(address.to_string(), account.clone());
        }

        Ok(account)
    }

    /// Process token transfers in a block and update balances
    pub async fn process_token_transfers_with_balances(
        &self,
        transfers: &[TokenTransfer],
        block_number: i64,
    ) -> Result<()> {
        if transfers.is_empty() {
            debug!("No token transfers to process for block {}", block_number);
            return Ok(());
        }

        if self.token_service.is_none() {
            warn!("Token service not available, skipping token balance processing");
            return Ok(());
        }

        let token_service = self.token_service.as_ref().unwrap();
        let mut token_updates = Vec::new();

        for transfer in transfers.iter() {
            // Discover token if not seen before
            if let Err(e) = token_service
                .discover_token(&transfer.token_address, block_number)
                .await
            {
                debug!("Failed to discover token {}: {}", transfer.token_address, e);
            } else {
                debug!("Token discovery completed for {}", transfer.token_address);
            }

            // Collect accounts that need balance updates
            token_updates.push((
                transfer.token_address.clone(),
                transfer.from_address.clone(),
                transfer.to_address.clone(),
            ));
        }

        debug!(
            "Collected {} token balance updates for block {}",
            token_updates.len(),
            block_number
        );

        // Update token balances
        if let Err(e) = token_service
            .update_balances_for_transfers(&token_updates, block_number)
            .await
        {
            error!("Failed to update token balances: {}", e);
        } else {
            debug!(
                "Successfully updated token balances for {} transfers in block {}",
                token_updates.len(),
                block_number
            );
        }

        Ok(())
    }
}
