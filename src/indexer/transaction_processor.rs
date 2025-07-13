use crate::{
    database::{Account, DatabaseService, Log, TokenTransfer, Transaction},
    rpc::RpcClient,
};
use anyhow::{Context, Result};
use ethers::core::types::{Log as EthLog, Transaction as EthTransaction, TransactionReceipt, H160};
use std::{collections::HashMap, str::FromStr, sync::Arc};
use tokio::sync::RwLock;
use tracing::{debug, error, trace};

// ERC-20 Transfer event topic
const TRANSFER_TOPIC: &str = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";

/// Processor for handling transaction data
#[derive(Clone)]
pub struct TransactionProcessor {
    db: Arc<DatabaseService>,
    rpc: Arc<RpcClient>,
    account_cache: Arc<RwLock<HashMap<String, Option<Account>>>>,
}

impl TransactionProcessor {
    /// Create a new transaction processor
    pub fn new(db: Arc<DatabaseService>, rpc: Arc<RpcClient>) -> Self {
        Self { 
            db, 
            rpc,
            account_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Process a transaction and its logs
    pub async fn process_transaction(&self, eth_tx: &EthTransaction) -> Result<()> {
        let tx_hash = format!("{:?}", eth_tx.hash);
        debug!("Processing transaction {}", tx_hash);

        // Get transaction receipt
        let receipt = self
            .rpc
            .get_transaction_receipt(&tx_hash)
            .await?
            .context(format!("Receipt for transaction {} not found", tx_hash))?;

        // Convert to our Transaction model and save
        let tx = self.convert_transaction(eth_tx, &receipt)?;
        self.db.insert_transaction(&tx).await?;

        // Process transaction logs
        for eth_log in &receipt.logs {
            if let Err(e) = self.process_transaction_log(&tx, eth_log).await {
                error!("Failed to process log: {}", e);
                // Continue with other logs
            }
        }

        // Update account balances (from and to addresses)
        let from_address = format!("{:#x}", eth_tx.from);
        if let Err(e) = self
            .update_account_balance(&from_address, tx.block_number)
            .await
        {
            error!("Failed to update from account balance: {}", e);
        }

        if let Some(to_addr) = eth_tx.to {
            let to_address = format!("{:#x}", to_addr);
            if let Err(e) = self.update_account_balance(&to_address, tx.block_number).await {
                error!("Failed to update to account balance: {}", e);
            }
        }

        debug!("Completed processing transaction {}", tx_hash);
        Ok(())
    }

    /// Get transaction receipts in batch for better performance
    pub async fn get_transaction_receipts_batch(&self, tx_hashes: &[String]) -> Result<Vec<Option<TransactionReceipt>>> {
        use futures::future;
        
        // Create semaphore to limit concurrent requests
        let semaphore = Arc::new(tokio::sync::Semaphore::new(50));
        
        // Create tasks for all receipt requests
        let tasks: Vec<_> = tx_hashes.iter().map(|hash| {
            let rpc = self.rpc.clone();
            let hash = hash.clone();
            let semaphore = semaphore.clone();
            
            async move {
                let _permit = semaphore.acquire().await?;
                rpc.get_transaction_receipt(&hash).await
            }
        }).collect();
        
        // Execute all requests concurrently
        let results = future::try_join_all(tasks).await?;
        Ok(results)
    }

    /// Collect transaction data for batch processing
    pub async fn collect_transaction_data(&self, eth_tx: &EthTransaction, receipt: &TransactionReceipt) -> Result<(Transaction, Vec<Log>, Vec<TokenTransfer>, Vec<Account>)> {
        // Convert to our Transaction model
        let tx = self.convert_transaction(eth_tx, receipt)?;
        
        // Collect all data
        let mut logs = Vec::new();
        let mut token_transfers = Vec::new();
        let mut accounts = Vec::new();

        // Process transaction logs
        for eth_log in &receipt.logs {
            // Convert log
            let log = self.convert_log(&tx, eth_log)?;
            logs.push(log);

            // Check if it's a token transfer
            if eth_log.topics.len() >= 3 && 
               format!("0x{}", hex::encode(eth_log.topics[0].as_bytes())) == "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef" {
                if let Ok(transfer) = self.process_erc20_transfer(&tx, eth_log).await {
                    token_transfers.push(transfer);
                }
            }
        }

        // Prepare accounts
        let from_address = format!("{:#x}", eth_tx.from);
        if let Ok(from_account) = self.prepare_account(&from_address, tx.block_number).await {
            accounts.push(from_account);
        }

        if let Some(to_addr) = eth_tx.to {
            let to_address = format!("{:#x}", to_addr);
            if let Ok(to_account) = self.prepare_account(&to_address, tx.block_number).await {
                accounts.push(to_account);
            }
        }

        Ok((tx, logs, token_transfers, accounts))
    }

    /// Collect data for multiple transactions efficiently (block-level batch processing)
    pub async fn collect_block_transaction_data(&self, transactions_with_receipts: &[(EthTransaction, TransactionReceipt)]) -> Result<(Vec<Transaction>, Vec<Log>, Vec<TokenTransfer>, Vec<Account>)> {
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
                if eth_log.topics.len() >= 3 && 
                   format!("0x{}", hex::encode(eth_log.topics[0].as_bytes())) == "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef" {
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
        let mut all_accounts = Vec::new();
        for address in unique_addresses {
            // Use the first transaction's block number as reference
            if let Some((first_tx, _)) = transactions_with_receipts.first() {
                let block_number = first_tx.block_number.map(|n| n.as_u64() as i64).unwrap_or(0);
                if let Ok(account) = self.prepare_account(&address, block_number).await {
                    all_accounts.push(account);
                }
            }
        }

        Ok((all_transactions, all_logs, all_token_transfers, all_accounts))
    }

    /// Process individual transaction log
    async fn process_transaction_log(&self, tx: &Transaction, eth_log: &EthLog) -> Result<()> {
        Self::process_log_static(&self.db, tx, eth_log).await
    }

    /// Static version of process_log for use in async tasks
    async fn process_log_static(db: &Arc<DatabaseService>, tx: &Transaction, eth_log: &EthLog) -> Result<()> {
        // Convert log to our model
        let log = Log {
            id: None,
            transaction_hash: tx.hash.clone(),
            log_index: eth_log.log_index.map(|i| i.as_u64() as i64).unwrap_or(0),
            address: format!("{:#x}", eth_log.address),
            topic0: eth_log.topics.get(0).map(|t| format!("{:#x}", t)),
            topic1: eth_log.topics.get(1).map(|t| format!("{:#x}", t)),
            topic2: eth_log.topics.get(2).map(|t| format!("{:#x}", t)),
            topic3: eth_log.topics.get(3).map(|t| format!("{:#x}", t)),
            data: Some(format!("{:#x}", eth_log.data)),
            block_number: tx.block_number,
        };

        db.insert_log(&log).await?;

        // Check if this is an ERC-20 transfer event
        if let Some(topic0) = &log.topic0 {
            if topic0 == TRANSFER_TOPIC && eth_log.topics.len() >= 3 {
                if let Err(e) = Self::process_token_transfer_static(db, &log, eth_log).await {
                    error!("Failed to process token transfer: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Static version of process_token_transfer for use in async tasks
    async fn process_token_transfer_static(db: &Arc<DatabaseService>, log: &Log, eth_log: &EthLog) -> Result<()> {
        // Extract from and to addresses from topics
        let from_address = if eth_log.topics.len() > 1 {
            let topic = eth_log.topics[1];
            format!("{:#x}", H160::from_slice(&topic.as_bytes()[12..32]))
        } else {
            return Ok(());
        };

        let to_address = if eth_log.topics.len() > 2 {
            let topic = eth_log.topics[2];
            format!("{:#x}", H160::from_slice(&topic.as_bytes()[12..32]))
        } else {
            return Ok(());
        };

        // Extract amount from data
        let amount = if !eth_log.data.0.is_empty() {
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
            transaction_hash: log.transaction_hash.clone(),
            token_address: log.address.clone(),
            from_address,
            to_address,
            amount,
            block_number: log.block_number,
            token_type: Some("ERC20".to_string()),
            token_id: None,
        };

        db.insert_token_transfer(&transfer).await?;
        Ok(())
    }

    /// Process ERC20 transfer from log
    async fn process_erc20_transfer(&self, tx: &Transaction, eth_log: &EthLog) -> Result<TokenTransfer> {
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

    /// Update account balance
    async fn update_account_balance(&self, address: &str, block_number: i64) -> Result<()> {
        // Get current balance from RPC
        let balance = self
            .rpc
            .get_balance(address, Some(block_number as u64))
            .await?;

        // Get existing account or create new one
        let existing_account = self.db.get_account_by_address(address).await?;

        let account = match existing_account {
            Some(mut acc) => {
                acc.balance = balance;
                acc.transaction_count += 1;
                acc.last_seen_block = block_number;
                acc
            }
            None => Account {
                address: address.to_string(),
                balance,
                transaction_count: 1,
                first_seen_block: block_number,
                last_seen_block: block_number,
            },
        };

        // Update in database
        self.db.update_account(&account).await?;

        Ok(())
    }

    /// Ensure that an account exists in the database, create if not
    async fn ensure_account_exists(&self, address: &str) -> Result<()> {
        let existing_account = self.db.get_account_by_address(address).await?;

        if existing_account.is_none() {
            // Create a new account with zero balance and transaction count
            let new_account = Account {
                address: address.to_string(),
                balance: "0".to_string(),
                transaction_count: 0,
                first_seen_block: 0,
                last_seen_block: 0,
            };

            self.db.update_account(&new_account).await?;
        }

        Ok(())
    }

    /// Prepare account for batch insertion
    async fn prepare_account(&self, address: &str, block_number: i64) -> Result<Account> {
        // Get current balance from RPC
        let balance = self
            .rpc
            .get_balance(address, Some(block_number as u64))
            .await
            .unwrap_or_default();

        // Get existing account or create new one using cache
        let existing_account = self.get_account_cached(address).await?;

        let account = if let Some(mut existing) = existing_account {
            existing.balance = balance.to_string();
            existing.transaction_count += 1;
            existing.last_seen_block = block_number;
            existing
        } else {
            Account {
                address: address.to_string(),
                balance: balance.to_string(),
                transaction_count: 1,
                first_seen_block: block_number,
                last_seen_block: block_number,
            }
        };

        Ok(account)
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

    /// Clear account cache (useful when processing in batches)
    pub async fn clear_account_cache(&self) {
        let mut cache = self.account_cache.write().await;
        cache.clear();
    }
}
