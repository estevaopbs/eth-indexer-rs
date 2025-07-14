mod models;

use anyhow::{Context, Result};
use sqlx::{
    migrate::MigrateDatabase, pool::PoolOptions, sqlite::SqlitePool, Executor, Pool, Sqlite,
};
use std::path::Path;
use tracing::{info, warn};

pub use models::*;

/// Service for database operations
pub struct DatabaseService {
    pub pool: Pool<Sqlite>,
}

impl DatabaseService {
    /// Create a new database service
    pub async fn new(database_url: &str) -> Result<Self> {
        let clean_url = database_url
            .strip_prefix("sqlite:")
            .unwrap_or(database_url)
            .to_string();

        // Create database directory if needed
        if let Some(db_path) = Path::new(&clean_url).parent() {
            if !db_path.exists() {
                std::fs::create_dir_all(db_path).context("Failed to create database directory")?;
                info!("Created database directory: {:?}", db_path);
            }
        }

        // Check if database exists, create if not
        if !Sqlite::database_exists(&clean_url).await.unwrap_or(false) {
            info!("Database does not exist, creating...");
            Sqlite::create_database(&clean_url)
                .await
                .context("Failed to create database")?;
        }

        // Connect to the database
        let pool = PoolOptions::new()
            .max_connections(10)
            .connect(&clean_url)
            .await
            .context("Failed to connect to database")?;

        // Run migrations
        info!("Running database migrations...");
        sqlx::migrate!("./src/database/migrations")
            .run(&pool)
            .await
            .context("Failed to run migrations")?;

        info!("Database initialized successfully");
        Ok(Self { pool })
    }

    /// Insert a new block
    pub async fn insert_block(&self, block: &Block) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO blocks (
                number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count,
                miner, total_difficulty, size_bytes, base_fee_per_gas, extra_data, state_root,
                nonce, withdrawals_root, blob_gas_used, excess_blob_gas, withdrawal_count,
                slot, proposer_index, epoch, slot_root, parent_root, beacon_deposit_count,
                graffiti, randao_reveal, randao_mix
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(number) DO UPDATE SET
                hash = excluded.hash,
                parent_hash = excluded.parent_hash,
                timestamp = excluded.timestamp,
                gas_used = excluded.gas_used,
                gas_limit = excluded.gas_limit,
                transaction_count = excluded.transaction_count,
                miner = excluded.miner,
                total_difficulty = excluded.total_difficulty,
                size_bytes = excluded.size_bytes,
                base_fee_per_gas = excluded.base_fee_per_gas,
                extra_data = excluded.extra_data,
                state_root = excluded.state_root,
                nonce = excluded.nonce,
                withdrawals_root = excluded.withdrawals_root,
                blob_gas_used = excluded.blob_gas_used,
                excess_blob_gas = excluded.excess_blob_gas,
                withdrawal_count = excluded.withdrawal_count,
                slot = excluded.slot,
                proposer_index = excluded.proposer_index,
                epoch = excluded.epoch,
                slot_root = excluded.slot_root,
                parent_root = excluded.parent_root,
                beacon_deposit_count = excluded.beacon_deposit_count,
                graffiti = excluded.graffiti,
                randao_reveal = excluded.randao_reveal,
                randao_mix = excluded.randao_mix
            "#,
        )
        .bind(block.number)
        .bind(&block.hash)
        .bind(&block.parent_hash)
        .bind(block.timestamp)
        .bind(block.gas_used)
        .bind(block.gas_limit)
        .bind(block.transaction_count)
        .bind(&block.miner)
        .bind(&block.total_difficulty)
        .bind(block.size_bytes)
        .bind(&block.base_fee_per_gas)
        .bind(&block.extra_data)
        .bind(&block.state_root)
        .bind(&block.nonce)
        .bind(&block.withdrawals_root)
        .bind(block.blob_gas_used)
        .bind(block.excess_blob_gas)
        .bind(block.withdrawal_count)
        .bind(block.slot)
        .bind(block.proposer_index)
        .bind(block.epoch)
        .bind(&block.slot_root)
        .bind(&block.parent_root)
        .bind(block.beacon_deposit_count)
        .bind(&block.graffiti)
        .bind(&block.randao_reveal)
        .bind(&block.randao_mix)
        .execute(&self.pool)
        .await
        .context("Failed to insert block")?;

        Ok(())
    }

    /// Insert a new transaction
    pub async fn insert_transaction(&self, tx: &Transaction) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO transactions (
                hash, block_number, from_address, to_address, value, gas_used, gas_price, status, transaction_index
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(hash) DO UPDATE SET
                block_number = excluded.block_number,
                from_address = excluded.from_address,
                to_address = excluded.to_address,
                value = excluded.value,
                gas_used = excluded.gas_used,
                gas_price = excluded.gas_price,
                status = excluded.status,
                transaction_index = excluded.transaction_index
            "#,
        )
        .bind(&tx.hash)
        .bind(tx.block_number)
        .bind(&tx.from_address)
        .bind(&tx.to_address)
        .bind(&tx.value)
        .bind(tx.gas_used)
        .bind(&tx.gas_price)
        .bind(tx.status)
        .bind(tx.transaction_index)
        .execute(&self.pool)
        .await
        .context("Failed to insert transaction")?;

        Ok(())
    }

    /// Insert a new log
    pub async fn insert_log(&self, log: &Log) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO logs (
                transaction_hash, block_number, address, topic0, topic1, topic2, topic3, data, log_index
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&log.transaction_hash)
        .bind(log.block_number)
        .bind(&log.address)
        .bind(&log.topic0)
        .bind(&log.topic1)
        .bind(&log.topic2)
        .bind(&log.topic3)
        .bind(&log.data)
        .bind(log.log_index)
        .execute(&self.pool)
        .await
        .context("Failed to insert log")?;

        Ok(())
    }

    /// Update or insert account information (upsert)
    pub async fn update_account(&self, account: &Account) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO accounts (
                address, balance, transaction_count, first_seen_block, last_seen_block
            ) VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(address) DO UPDATE SET
                balance = excluded.balance,
                transaction_count = excluded.transaction_count,
                last_seen_block = excluded.last_seen_block,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(&account.address)
        .bind(&account.balance)
        .bind(account.transaction_count)
        .bind(account.first_seen_block)
        .bind(account.last_seen_block)
        .execute(&self.pool)
        .await
        .context("Failed to update account")?;

        Ok(())
    }

    /// Insert a new withdrawal
    pub async fn insert_withdrawal(&self, withdrawal: &Withdrawal) -> Result<()> {
        // First check if withdrawal already exists
        let existing = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM withdrawals WHERE block_number = ? AND withdrawal_index = ?",
        )
        .bind(withdrawal.block_number)
        .bind(withdrawal.withdrawal_index)
        .fetch_one(&self.pool)
        .await
        .context("Failed to check existing withdrawal")?;

        // Only insert if it doesn't exist
        if existing == 0 {
            sqlx::query(
                r#"
                INSERT INTO withdrawals (
                    block_number, withdrawal_index, validator_index, address, amount
                ) VALUES (?, ?, ?, ?, ?)
                "#,
            )
            .bind(withdrawal.block_number)
            .bind(withdrawal.withdrawal_index)
            .bind(withdrawal.validator_index)
            .bind(&withdrawal.address)
            .bind(&withdrawal.amount)
            .execute(&self.pool)
            .await
            .context("Failed to insert withdrawal")?;
        }

        Ok(())
    }

    /// Insert a new token transfer
    pub async fn insert_token_transfer(&self, token_transfer: &TokenTransfer) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO token_transfers (
                transaction_hash, block_number, token_address, from_address, to_address, amount, token_type, token_id
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&token_transfer.transaction_hash)
        .bind(token_transfer.block_number)
        .bind(&token_transfer.token_address)
        .bind(&token_transfer.from_address)
        .bind(&token_transfer.to_address)
        .bind(&token_transfer.amount)
        .bind(&token_transfer.token_type)
        .bind(&token_transfer.token_id)
        .execute(&self.pool)
        .await
        .context("Failed to insert token transfer")?;

        Ok(())
    }

    /// Insert multiple transactions in a single batch for better performance
    pub async fn insert_transactions_batch(&self, transactions: &[Transaction]) -> Result<()> {
        if transactions.is_empty() {
            return Ok(());
        }

        let mut query_builder = sqlx::QueryBuilder::new(
            "INSERT INTO transactions (hash, block_number, transaction_index, from_address, to_address, value, gas_used, gas_price, status) "
        );

        query_builder.push_values(transactions, |mut b, tx| {
            b.push_bind(&tx.hash)
                .push_bind(tx.block_number)
                .push_bind(tx.transaction_index)
                .push_bind(&tx.from_address)
                .push_bind(&tx.to_address)
                .push_bind(&tx.value)
                .push_bind(tx.gas_used)
                .push_bind(&tx.gas_price)
                .push_bind(tx.status);
        });

        query_builder.build().execute(&self.pool).await?;
        Ok(())
    }

    /// Insert multiple logs in a single batch for better performance
    pub async fn insert_logs_batch(&self, logs: &[Log]) -> Result<()> {
        if logs.is_empty() {
            return Ok(());
        }

        let mut query_builder = sqlx::QueryBuilder::new(
            "INSERT INTO logs (transaction_hash, log_index, address, topic0, topic1, topic2, topic3, data, block_number) "
        );

        query_builder.push_values(logs, |mut b, log| {
            b.push_bind(&log.transaction_hash)
                .push_bind(log.log_index)
                .push_bind(&log.address)
                .push_bind(&log.topic0)
                .push_bind(&log.topic1)
                .push_bind(&log.topic2)
                .push_bind(&log.topic3)
                .push_bind(&log.data)
                .push_bind(log.block_number);
        });

        query_builder.build().execute(&self.pool).await?;
        Ok(())
    }

    /// Insert multiple token transfers in a single batch for better performance
    pub async fn insert_token_transfers_batch(&self, transfers: &[TokenTransfer]) -> Result<()> {
        if transfers.is_empty() {
            return Ok(());
        }

        let mut query_builder = sqlx::QueryBuilder::new(
            "INSERT INTO token_transfers (transaction_hash, token_address, from_address, to_address, amount, block_number, token_type, token_id) "
        );

        query_builder.push_values(transfers, |mut b, transfer| {
            b.push_bind(&transfer.transaction_hash)
                .push_bind(&transfer.token_address)
                .push_bind(&transfer.from_address)
                .push_bind(&transfer.to_address)
                .push_bind(&transfer.amount)
                .push_bind(transfer.block_number)
                .push_bind(&transfer.token_type)
                .push_bind(&transfer.token_id);
        });

        query_builder.build().execute(&self.pool).await?;
        Ok(())
    }

    /// Insert multiple accounts in a single batch for better performance
    pub async fn insert_accounts_batch(&self, accounts: &[Account]) -> Result<()> {
        if accounts.is_empty() {
            return Ok(());
        }

        let mut query_builder = sqlx::QueryBuilder::new(
            "INSERT OR IGNORE INTO accounts (address, balance, transaction_count, first_seen_block, last_seen_block) "
        );

        query_builder.push_values(accounts, |mut b, account| {
            b.push_bind(&account.address)
                .push_bind(&account.balance)
                .push_bind(account.transaction_count)
                .push_bind(account.first_seen_block)
                .push_bind(account.last_seen_block);
        });

        query_builder.build().execute(&self.pool).await?;
        Ok(())
    }

    /// Get the latest block number
    pub async fn get_latest_block_number(&self) -> Result<Option<i64>> {
        let result: (Option<i64>,) = sqlx::query_as("SELECT MAX(number) FROM blocks")
            .fetch_one(&self.pool)
            .await
            .context("Failed to query latest block number")?;

        Ok(result.0)
    }

    /// Get block by number
    pub async fn get_block_by_number(&self, number: i64) -> Result<Option<Block>> {
        let result = sqlx::query_as::<_, Block>(
            r#"
            SELECT number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count,
                   miner, total_difficulty, size_bytes, base_fee_per_gas, extra_data, state_root,
                   nonce, withdrawals_root, blob_gas_used, excess_blob_gas, withdrawal_count,
                   slot, proposer_index, epoch, slot_root, parent_root, beacon_deposit_count,
                   graffiti, randao_reveal, randao_mix
            FROM blocks
            WHERE number = ?
            "#,
        )
        .bind(number)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to query block by number")?;

        Ok(result)
    }

    /// Get block by hash
    pub async fn get_block_by_hash(&self, hash: &str) -> Result<Option<Block>> {
        let result = sqlx::query_as::<_, Block>(
            r#"
            SELECT number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count,
                   miner, total_difficulty, size_bytes, base_fee_per_gas, extra_data, state_root,
                   nonce, withdrawals_root, blob_gas_used, excess_blob_gas, withdrawal_count,
                   slot, proposer_index, epoch, slot_root, parent_root, beacon_deposit_count,
                   graffiti, randao_reveal, randao_mix
            FROM blocks
            WHERE hash = ?
            "#,
        )
        .bind(hash)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to query block by hash")?;

        Ok(result)
    }

    /// Get transactions by block number
    pub async fn get_transactions_by_block(&self, block_number: i64) -> Result<Vec<Transaction>> {
        let result = sqlx::query_as::<_, Transaction>(
            r#"
            SELECT hash, block_number, from_address, to_address, value, gas_used, gas_price, status, transaction_index
            FROM transactions
            WHERE block_number = ?
            ORDER BY transaction_index
            "#,
        )
        .bind(block_number)
        .fetch_all(&self.pool)
        .await
        .context("Failed to query transactions by block")?;

        Ok(result)
    }

    /// Get transaction by hash
    pub async fn get_transaction_by_hash(&self, hash: &str) -> Result<Option<Transaction>> {
        let result = sqlx::query_as::<_, Transaction>(
            r#"
            SELECT hash, block_number, from_address, to_address, value, gas_used, gas_price, status, transaction_index
            FROM transactions
            WHERE hash = ?
            "#,
        )
        .bind(hash)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to query transaction by hash")?;

        Ok(result)
    }

    /// Get logs by transaction hash
    pub async fn get_logs_by_transaction(&self, tx_hash: &str) -> Result<Vec<Log>> {
        let result = sqlx::query_as::<_, Log>(
            r#"
            SELECT id, transaction_hash, block_number, address, topic0, topic1, topic2, topic3, data, log_index
            FROM logs
            WHERE transaction_hash = ?
            ORDER BY log_index
            "#,
        )
        .bind(tx_hash)
        .fetch_all(&self.pool)
        .await
        .context("Failed to query logs by transaction")?;

        Ok(result)
    }

    /// Get account by address
    pub async fn get_account_by_address(&self, address: &str) -> Result<Option<Account>> {
        let result = sqlx::query_as::<_, Account>(
            r#"
            SELECT address, balance, transaction_count, first_seen_block, last_seen_block
            FROM accounts
            WHERE address = ?
            "#,
        )
        .bind(address)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to query account by address")?;

        Ok(result)
    }

    /// Get recent blocks with pagination
    pub async fn get_recent_blocks(&self, limit: i64, offset: i64) -> Result<Vec<Block>> {
        let result = sqlx::query_as::<_, Block>(
            r#"
            SELECT 
                number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count,
                miner, total_difficulty, size_bytes, base_fee_per_gas, extra_data, state_root,
                nonce, withdrawals_root, blob_gas_used, excess_blob_gas, withdrawal_count,
                slot, proposer_index, epoch, slot_root, parent_root, beacon_deposit_count,
                graffiti, randao_reveal, randao_mix
            FROM blocks
            ORDER BY number DESC
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .context("Failed to query recent blocks")?;

        Ok(result)
    }

    /// Get recent transactions with pagination
    pub async fn get_recent_transactions(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Transaction>> {
        let result = sqlx::query_as::<_, Transaction>(
            r#"
            SELECT hash, block_number, from_address, to_address, value, gas_used, gas_price, status, transaction_index
            FROM transactions
            ORDER BY block_number DESC, transaction_index DESC
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .context("Failed to query recent transactions")?;

        Ok(result)
    }

    /// Get total number of blocks
    pub async fn get_block_count(&self) -> Result<i64> {
        let result: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM blocks")
            .fetch_one(&self.pool)
            .await
            .context("Failed to query block count")?;

        Ok(result.0)
    }

    /// Get total number of transactions
    pub async fn get_transaction_count(&self) -> Result<i64> {
        let result: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transactions")
            .fetch_one(&self.pool)
            .await
            .context("Failed to query transaction count")?;

        Ok(result.0)
    }

    /// Get total declared transactions from blocks (sum of transaction_count field)
    pub async fn get_declared_transaction_count(&self) -> Result<i64> {
        let result: (Option<i64>,) = sqlx::query_as("SELECT SUM(transaction_count) FROM blocks")
            .fetch_one(&self.pool)
            .await
            .context("Failed to query declared transaction count")?;

        Ok(result.0.unwrap_or(0))
    }

    /// Get historical transaction count before start block
    /// This estimates the total transactions that existed before our indexing started
    pub async fn get_historical_transaction_count(&self, start_block: u64) -> Result<i64> {
        // For now, use fallback estimation until we integrate with RpcClient
        let estimated_count = match start_block {
            0..=1000000 => 0,                     // Genesis to early 2016
            1000001..=4000000 => 50_000_000,      // 2016-2017: ~50M transactions
            4000001..=8000000 => 250_000_000,     // 2018-2019: ~250M transactions
            8000001..=12000000 => 750_000_000,    // 2020-2021: ~750M transactions
            12000001..=15000000 => 1_200_000_000, // 2021-2022: ~1.2B transactions
            15000001..=17000000 => 1_500_000_000, // 2022-2023: ~1.5B transactions
            17000001..=19000000 => 1_800_000_000, // 2023-2024: ~1.8B transactions
            19000001..=20000000 => 2_200_000_000, // 2024: ~2.2B transactions
            _ => {
                // For blocks after 20M, estimate ~150 txs per block average
                let avg_txs_per_block = 150;
                (start_block as i64) * avg_txs_per_block
            }
        };

        Ok(estimated_count)
    }

    /// Get withdrawals for a block
    pub async fn get_withdrawals_by_block(&self, block_number: i64) -> Result<Vec<Withdrawal>> {
        let withdrawals = sqlx::query_as::<_, Withdrawal>(
            "SELECT * FROM withdrawals WHERE block_number = ? ORDER BY withdrawal_index",
        )
        .bind(block_number)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch withdrawals")?;

        Ok(withdrawals)
    }

    /// Get current block transaction information
    pub async fn get_current_block_transaction_info(&self) -> Result<(i64, i64)> {
        // Get the latest block number
        let latest_block = self.get_latest_block_number().await?.unwrap_or(-1);

        if latest_block < 0 {
            return Ok((0, 0));
        }

        // Get declared transaction count for current block
        let declared_result: (Option<i64>,) =
            sqlx::query_as("SELECT transaction_count FROM blocks WHERE number = ? LIMIT 1")
                .bind(latest_block)
                .fetch_one(&self.pool)
                .await
                .context("Failed to query current block transaction count")?;

        let declared_count = declared_result.0.unwrap_or(0);

        // Get indexed transaction count for current block
        let indexed_result: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE block_number = ?")
                .bind(latest_block)
                .fetch_one(&self.pool)
                .await
                .context("Failed to query current block indexed transactions")?;

        let indexed_count = indexed_result.0;

        Ok((indexed_count, declared_count))
    }

    /// Get cached historical transaction count for a specific block
    pub async fn get_cached_historical_count(&self, block_number: i64) -> Result<Option<i64>> {
        let result = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT total_transactions_before FROM historical_transaction_cache WHERE block_number = ?"
        )
        .bind(block_number)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get cached historical count")?;

        Ok(result.flatten())
    }

    /// Cache historical transaction count for a specific block
    pub async fn cache_historical_count(
        &self,
        block_number: i64,
        total_transactions_before: i64,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO historical_transaction_cache 
            (block_number, total_transactions_before) 
            VALUES (?, ?)
            "#,
        )
        .bind(block_number)
        .bind(total_transactions_before)
        .execute(&self.pool)
        .await
        .context("Failed to cache historical count")?;

        Ok(())
    }

    /// Get all cached historical counts ordered by block number
    pub async fn get_all_cached_historical_counts(&self) -> Result<Vec<(i64, i64)>> {
        let result = sqlx::query_as::<_, (i64, i64)>(
            "SELECT block_number, total_transactions_before FROM historical_transaction_cache ORDER BY block_number"
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to get all cached historical counts")?;

        Ok(result)
    }

    /// Get closest cached historical count to a target block
    pub async fn get_closest_cached_historical_count(
        &self,
        target_block: i64,
    ) -> Result<Option<(i64, i64)>> {
        let result = sqlx::query_as::<_, (i64, i64)>(
            r#"
            SELECT block_number, total_transactions_before 
            FROM historical_transaction_cache 
            ORDER BY ABS(block_number - ?) 
            LIMIT 1
            "#,
        )
        .bind(target_block)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get closest cached historical count")?;

        Ok(result)
    }

    /// Get a configuration value from the database
    pub async fn get_config(&self, key: &str) -> Result<Option<String>> {
        let result = sqlx::query_as::<_, (String,)>("SELECT value FROM config WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .context("Failed to get config value")?;

        Ok(result.map(|(value,)| value))
    }

    /// Set a configuration value in the database
    pub async fn set_config(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO config (key, value) 
            VALUES (?, ?)
            ON CONFLICT(key) DO UPDATE SET 
                value = excluded.value,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await
        .context("Failed to set config value")?;

        Ok(())
    }

    /// Get the start block from database configuration
    pub async fn get_start_block(&self) -> Result<Option<u64>> {
        if let Some(value_str) = self.get_config("start_block").await? {
            if value_str != "0" {
                let start_block = value_str
                    .parse::<u64>()
                    .context("Failed to parse start_block from database")?;
                return Ok(Some(start_block));
            }
        }
        Ok(None)
    }

    /// Set the start block in database configuration
    pub async fn set_start_block(&self, start_block: u64) -> Result<()> {
        self.set_config("start_block", &start_block.to_string())
            .await
    }
}
