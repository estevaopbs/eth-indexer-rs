use crate::{
    database::{Account, DatabaseService, Log, TokenTransfer, Transaction},
    rpc::RpcClient,
};
use anyhow::{Context, Result};
use ethers::core::types::{Log as EthLog, Transaction as EthTransaction, TransactionReceipt, H160};
use std::{str::FromStr, sync::Arc};
use tracing::{debug, error, trace};

// ERC-20 Transfer event topic
const TRANSFER_TOPIC: &str = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";

/// Processor for handling transaction data
pub struct TransactionProcessor {
    db: Arc<DatabaseService>,
    rpc: Arc<RpcClient>,
}

impl TransactionProcessor {
    /// Create a new transaction processor
    pub fn new(db: Arc<DatabaseService>, rpc: Arc<RpcClient>) -> Self {
        Self { db, rpc }
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
            if let Err(e) = self.process_log(&tx, eth_log).await {
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

    /// Process a transaction log
    async fn process_log(&self, tx: &Transaction, eth_log: &EthLog) -> Result<()> {
        let log = self.convert_log(tx, eth_log)?;
        self.db.insert_log(&log).await?;

        // Check if this is an ERC-20 transfer event
        if let Some(topic0) = &log.topic0 {
            if topic0 == TRANSFER_TOPIC {
                if let Err(e) = self.process_token_transfer(tx, eth_log).await {
                    error!("Failed to process token transfer: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Process ERC-20 token transfer
    async fn process_token_transfer(&self, tx: &Transaction, eth_log: &EthLog) -> Result<()> {
        // Extract token transfer data
        let token_address = format!("{:#x}", eth_log.address);

        // Topic1 is from address (padded to 32 bytes)
        let from_address = if eth_log.topics.len() > 1 {
            let bytes: [u8; 20] = eth_log.topics[1].as_fixed_bytes()[12..].try_into().unwrap();
            let from_h160 = H160::from(bytes);
            format!("{:#x}", from_h160)
        } else {
            return Err(anyhow::anyhow!(
                "Invalid Transfer event: missing from address"
            ));
        };

        // Topic2 is to address (padded to 32 bytes)
        let to_address = if eth_log.topics.len() > 2 {
            let bytes: [u8; 20] = eth_log.topics[2].as_fixed_bytes()[12..].try_into().unwrap();
            let to_h160 = H160::from(bytes);
            format!("{:#x}", to_h160)
        } else {
            return Err(anyhow::anyhow!(
                "Invalid Transfer event: missing to address"
            ));
        };

        // Amount is in the data field
        let amount = if eth_log.data.0.len() > 0 {
            ethers::core::types::U256::from_big_endian(&eth_log.data.0).to_string()
        } else {
            "0".to_string()
        };

        // Create token transfer record
        let token_transfer = TokenTransfer {
            id: None,
            transaction_hash: tx.hash.clone(),
            block_number: tx.block_number,
            token_address,
            from_address,
            to_address,
            amount,
            token_type: Some("ERC20".to_string()), // Default to ERC20
            token_id: None, // Not applicable for ERC20
        };

        // Save to database
        self.db.insert_token_transfer(&token_transfer).await?;
        trace!("Processed token transfer: {:?}", token_transfer);

        Ok(())
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
}
