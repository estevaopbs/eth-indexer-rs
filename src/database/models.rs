use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Block data structure
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Block {
    pub number: i64,
    pub hash: String,
    pub parent_hash: String,
    pub timestamp: i64,
    pub gas_used: i64,
    pub gas_limit: i64,
    pub transaction_count: i64,

    // Extended fields available from RPC
    pub miner: Option<String>,            // Fee recipient
    pub difficulty: Option<String>,       // Block difficulty (legacy, only available pre-merge)
    pub size_bytes: Option<i64>,          // Block size in bytes
    pub base_fee_per_gas: Option<String>, // Base fee per gas (EIP-1559)
    pub extra_data: Option<String>,       // Extra data
    pub state_root: Option<String>,       // State root hash
    pub nonce: Option<String>,            // Block nonce
    pub withdrawals_root: Option<String>, // Withdrawals root (Shanghai)
    pub blob_gas_used: Option<i64>,       // Blob gas used (EIP-4844)
    pub excess_blob_gas: Option<i64>,     // Excess blob gas (EIP-4844)
    pub withdrawal_count: Option<i64>,    // Number of withdrawals in block

    // Beacon Chain fields (requires separate API connection)
    pub slot: Option<i64>,                 // Beacon chain slot
    pub proposer_index: Option<i64>,       // Validator proposer index
    pub epoch: Option<i64>,                // Beacon chain epoch
    pub slot_root: Option<String>,         // Slot root hash
    pub parent_root: Option<String>,       // Parent root hash
    pub beacon_deposit_count: Option<i64>, // Beacon chain deposit count
    pub graffiti: Option<String>,          // Proposer graffiti
    pub randao_reveal: Option<String>,     // Randao reveal signature
    pub randao_mix: Option<String>,        // Block randomness
}

impl Block {
    /// Calculate burnt fees (base_fee * gas_used)
    pub fn burnt_fees(&self) -> Option<String> {
        if let Some(base_fee_str) = &self.base_fee_per_gas {
            if let Ok(base_fee_val) = base_fee_str.parse::<u64>() {
                return Some((base_fee_val * self.gas_used as u64).to_string());
            }
        }
        None
    }

    /// Calculate block reward placeholder (actual calculation needs transaction data)
    /// This returns None as the calculation requires access to transaction data
    /// Use BlockResponse::calculate_block_reward_with_transactions for full calculation
    pub fn block_reward(&self) -> Option<String> {
        // Block reward in Ethereum PoS consists of:
        // 1. Base validator reward (consensus layer)
        // 2. Priority fees (tips) from transactions
        // 3. MEV rewards (if applicable)
        //
        // Since we need transaction data, this method returns None
        // The actual calculation is done in BlockResponse
        None
    }

    /// Calculate gas utilization percentage
    pub fn gas_utilization(&self) -> f64 {
        if self.gas_limit > 0 {
            (self.gas_used as f64 / self.gas_limit as f64) * 100.0
        } else {
            0.0
        }
    }

    /// Calculate blob gas utilization percentage (EIP-4844)
    pub fn blob_utilization(&self) -> Option<f64> {
        if let Some(blob_gas_used) = self.blob_gas_used {
            // Maximum blob gas per block is 786,432 (6 blobs * 131,072 gas per blob)
            const MAX_BLOB_GAS_PER_BLOCK: i64 = 786_432;
            Some((blob_gas_used as f64 / MAX_BLOB_GAS_PER_BLOCK as f64) * 100.0)
        } else {
            None
        }
    }

    /// Get block status based on block age and network finality
    pub fn status(&self, latest_block: i64) -> String {
        let block_age = latest_block - self.number;

        if block_age >= 32 {
            // Block is older than 32 blocks (2 epochs), considered finalized
            "finalized".to_string()
        } else if block_age >= 12 {
            // Block is older than 12 blocks, considered safe
            "safe".to_string()
        } else if block_age >= 1 {
            // Block has at least 1 confirmation
            "pending".to_string()
        } else {
            // Block is the latest block
            "latest".to_string()
        }
    }

    /// Check if block has withdrawals (post-Shanghai)
    pub fn has_withdrawals(&self) -> bool {
        self.withdrawals_root.is_some() && self.withdrawal_count.unwrap_or(0) > 0
    }

    /// Check if block uses EIP-4844 blobs
    pub fn has_blobs(&self) -> bool {
        self.blob_gas_used.is_some() && self.blob_gas_used.unwrap_or(0) > 0
    }

    /// Calculate blob transactions count (transactions using blob gas)
    pub fn blob_transactions_count(&self, transactions: &[Transaction]) -> i64 {
        // In a real implementation, we'd need to check transaction type
        // For now, estimate based on blob gas usage
        if self.has_blobs() && !transactions.is_empty() {
            // Rough estimate: if block has blob gas, assume some transactions are blob txs
            // This would need proper transaction type checking in a full implementation
            (transactions.len() as f64 * 0.1).ceil() as i64
        } else {
            0
        }
    }

    /// Calculate total blob size in bytes
    pub fn blob_size(&self) -> Option<i64> {
        if let Some(blob_gas_used) = self.blob_gas_used {
            // Each blob is 131,072 bytes, and each byte uses ~1 gas
            // This is a simplified calculation
            Some(blob_gas_used / 1024) // Convert gas to approximate KB
        } else {
            None
        }
    }

    /// Calculate current blob gas price (EIP-4844)
    pub fn blob_gas_price(&self) -> Option<String> {
        if let Some(excess_blob_gas) = self.excess_blob_gas {
            // Blob gas price calculation per EIP-4844
            // price = MIN_BLOB_GASPRICE * e^(excess_blob_gas / BLOB_GASPRICE_UPDATE_FRACTION)
            const MIN_BLOB_GASPRICE: f64 = 1.0;
            const BLOB_GASPRICE_UPDATE_FRACTION: f64 = 3_338_477.0;

            let price =
                MIN_BLOB_GASPRICE * (excess_blob_gas as f64 / BLOB_GASPRICE_UPDATE_FRACTION).exp();
            Some(price.round() as u64).map(|p| p.to_string())
        } else {
            None
        }
    }
}

/// Transaction data structure
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Transaction {
    pub hash: String,
    pub block_number: i64,
    pub from_address: String,
    pub to_address: Option<String>,
    pub value: String,
    pub gas_used: i64,
    pub gas_price: String,
    pub status: i64,
    pub transaction_index: i64,
}

/// Log data structure
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Log {
    #[sqlx(default)]
    pub id: Option<i64>,
    pub transaction_hash: String,
    pub block_number: i64,
    pub address: String,
    pub topic0: Option<String>,
    pub topic1: Option<String>,
    pub topic2: Option<String>,
    pub topic3: Option<String>,
    pub data: Option<String>,
    pub log_index: i64,
}

/// Account data structure
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Account {
    pub address: String,
    pub balance: String,
    pub transaction_count: i64,
    pub first_seen_block: i64,
    pub last_seen_block: i64,
}

/// Token transfer data structure
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct TokenTransfer {
    #[sqlx(default)]
    pub id: Option<i64>,
    pub transaction_hash: String,
    pub block_number: i64,
    pub token_address: String,
    pub from_address: String,
    pub to_address: String,
    pub amount: String,
    #[sqlx(default)]
    pub token_type: Option<String>, // ERC20, ERC721, ERC1155
    #[sqlx(default)]
    pub token_id: Option<String>, // For NFTs
}

/// Token information structure
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Token {
    pub address: String,
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub decimals: Option<u8>,
    pub token_type: String, // ERC20, ERC721, ERC1155
    pub first_seen_block: i64,
    pub last_seen_block: i64,
    pub total_transfers: i64,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// Token balance structure for storing account token balances
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct TokenBalance {
    #[sqlx(default)]
    pub id: Option<i64>,
    pub account_address: String,
    pub token_address: String,
    pub balance: String,
    pub block_number: i64,
    pub last_updated_block: i64,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// Stats structure for API responses
#[derive(Debug, Serialize, Deserialize)]
pub struct IndexerStats {
    pub latest_block: i64,
    pub total_blocks: i64,
    pub total_transactions: i64,
    pub total_transactions_declared: i64,
    pub total_transactions_indexed: i64,
    pub real_transactions_indexed: i64, // Only transactions from start_block onwards
    pub total_blockchain_transactions: i64, // Historical + indexed transactions
    pub total_accounts: i64,
    pub indexer_status: String,
    pub sync_percentage: f64,
    pub transaction_indexing_percentage: f64,
    pub start_block: i64,
    pub current_block_tx_indexed: i64,
    pub current_block_tx_declared: i64,
}

/// Pagination parameters
#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

impl PaginationParams {
    pub fn limit(&self) -> i64 {
        self.per_page.unwrap_or(10).min(100) as i64
    }

    pub fn offset(&self) -> i64 {
        (self.page.unwrap_or(1).saturating_sub(1) * self.per_page.unwrap_or(10)) as i64
    }
}

/// Transaction filter parameters
#[derive(Debug, Deserialize)]
pub struct TransactionFilterParams {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub status: Option<String>,    // "success", "failed", or "all"
    pub min_value: Option<String>, // minimum value in Wei
    pub max_value: Option<String>, // maximum value in Wei
    pub from_block: Option<i64>,   // minimum block number
    pub to_block: Option<i64>,     // maximum block number
}

impl TransactionFilterParams {
    pub fn limit(&self) -> i64 {
        self.per_page.unwrap_or(10).min(100) as i64
    }

    pub fn offset(&self) -> i64 {
        (self.page.unwrap_or(1).saturating_sub(1) * self.per_page.unwrap_or(10)) as i64
    }
}

/// Account filter parameters
#[derive(Debug, Deserialize)]
pub struct AccountFilterParams {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub account_type: Option<String>, // "eoa", "contract", "unknown", or "all"
    pub min_balance: Option<String>,  // minimum balance in Wei
    pub max_balance: Option<String>,  // maximum balance in Wei
    pub min_tx_count: Option<i64>,    // minimum transaction count
    pub max_tx_count: Option<i64>,    // maximum transaction count
    pub sort: Option<String>,         // "balance", "tx_count", "first_seen", "last_activity"
    pub order: Option<String>,        // "asc" or "desc"
}

impl AccountFilterParams {
    pub fn limit(&self) -> i64 {
        self.per_page.unwrap_or(10).min(100) as i64
    }

    pub fn offset(&self) -> i64 {
        (self.page.unwrap_or(1).saturating_sub(1) * self.per_page.unwrap_or(10)) as i64
    }
}

/// Block response structure for API with calculated fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockResponse {
    pub number: i64,
    pub hash: String,
    pub parent_hash: String,
    pub timestamp: i64,
    pub gas_used: i64,
    pub gas_limit: i64,
    pub transaction_count: i64,

    // Extended fields available from RPC
    pub miner: Option<String>,
    pub difficulty: Option<String>,
    pub size_bytes: Option<i64>,
    pub base_fee_per_gas: Option<String>,
    pub extra_data: Option<String>,
    pub state_root: Option<String>,
    pub nonce: Option<String>,
    pub withdrawals_root: Option<String>,
    pub blob_gas_used: Option<i64>,
    pub excess_blob_gas: Option<i64>,
    pub withdrawal_count: Option<i64>,

    // Beacon Chain fields
    pub slot: Option<i64>,
    pub proposer_index: Option<i64>,
    pub epoch: Option<i64>,
    pub slot_root: Option<String>,
    pub parent_root: Option<String>,
    pub beacon_deposit_count: Option<i64>,
    pub graffiti: Option<String>,
    pub randao_reveal: Option<String>,
    pub randao_mix: Option<String>,

    // Calculated fields
    pub burnt_fees: Option<String>,
    pub block_reward: Option<String>,
    pub status: String,
    pub gas_utilization: f64,
    pub blob_utilization: Option<f64>,
    pub priority_fees: Option<String>,
    pub blob_transactions: Option<i64>,
    pub blob_size: Option<i64>,
    pub blob_gas_price: Option<String>,
}

impl From<&Block> for BlockResponse {
    fn from(block: &Block) -> Self {
        Self {
            number: block.number,
            hash: block.hash.clone(),
            parent_hash: block.parent_hash.clone(),
            timestamp: block.timestamp,
            gas_used: block.gas_used,
            gas_limit: block.gas_limit,
            transaction_count: block.transaction_count,
            miner: block.miner.clone(),
            difficulty: block.difficulty.clone(),
            size_bytes: block.size_bytes,
            base_fee_per_gas: block.base_fee_per_gas.clone(),
            extra_data: block.extra_data.clone(),
            state_root: block.state_root.clone(),
            nonce: block.nonce.clone(),
            withdrawals_root: block.withdrawals_root.clone(),
            blob_gas_used: block.blob_gas_used,
            excess_blob_gas: block.excess_blob_gas,
            withdrawal_count: block.withdrawal_count,

            // Beacon Chain fields
            slot: block.slot,
            proposer_index: block.proposer_index,
            epoch: block.epoch,
            slot_root: block.slot_root.clone(),
            parent_root: block.parent_root.clone(),
            beacon_deposit_count: block.beacon_deposit_count,
            graffiti: block.graffiti.clone(),
            randao_reveal: block.randao_reveal.clone(),
            randao_mix: block.randao_mix.clone(),

            // Calculate fields dynamically (using defaults for now)
            burnt_fees: block.burnt_fees(),
            block_reward: block.block_reward(),
            status: "finalized".to_string(), // Will be updated with calculate_status
            gas_utilization: block.gas_utilization(),
            blob_utilization: block.blob_utilization(),
            priority_fees: None, // Calculated separately with transaction data
            blob_transactions: None, // Calculated separately with transaction data
            blob_size: block.blob_size(),
            blob_gas_price: block.blob_gas_price(),
        }
    }
}

impl BlockResponse {
    /// Calculate status based on latest block
    pub fn calculate_status(&mut self, latest_block: i64) {
        let block_age = latest_block - self.number;

        self.status = if block_age >= 32 {
            "finalized".to_string()
        } else if block_age >= 12 {
            "safe".to_string()
        } else if block_age >= 1 {
            "pending".to_string()
        } else {
            "latest".to_string()
        };
    }

    /// Calculate blob transactions count with transaction data
    pub fn calculate_blob_transactions(&mut self, transactions: &[Transaction]) {
        if self.blob_gas_used.is_some() && self.blob_gas_used.unwrap_or(0) > 0 {
            // In a real implementation, we'd check transaction type (type 3 = blob tx)
            // For now, estimate based on blob gas usage
            self.blob_transactions = Some((transactions.len() as f64 * 0.1).ceil() as i64);
        } else {
            self.blob_transactions = Some(0);
        }
    }

    /// Calculate block reward with transaction data
    /// This includes priority fees (tips) from all transactions in the block
    pub fn calculate_block_reward_with_transactions(&mut self, transactions: &[Transaction]) {
        let mut total_priority_fees = 0u64;

        if let Some(base_fee_str) = &self.base_fee_per_gas {
            if let Ok(base_fee) = base_fee_str.parse::<u64>() {
                for tx in transactions {
                    if let Ok(gas_price) = tx.gas_price.parse::<u64>() {
                        // Priority fee = gas_price - base_fee (for legacy transactions)
                        // For EIP-1559 transactions, this would be max_priority_fee_per_gas
                        if gas_price > base_fee {
                            let priority_fee = gas_price - base_fee;
                            total_priority_fees += priority_fee * tx.gas_used as u64;
                        }
                    }
                }
            }
        } else {
            // Pre-EIP-1559 blocks: all gas fees go to miner
            for tx in transactions {
                if let Ok(gas_price) = tx.gas_price.parse::<u64>() {
                    total_priority_fees += gas_price * tx.gas_used as u64;
                }
            }
        }

        self.priority_fees = Some(total_priority_fees.to_string());

        // Calculate total block reward
        // In PoS, block reward = base_validator_reward + priority_fees + MEV
        // For now, we only calculate priority fees (tips)
        // Base validator reward requires consensus layer data
        let block_reward = if total_priority_fees > 0 {
            // TODO: Add base validator reward and MEV when available
            total_priority_fees.to_string()
        } else {
            "0".to_string()
        };

        self.block_reward = Some(block_reward);
    }

    /// Calculate priority fees (tips) from transactions
    pub fn calculate_priority_fees(&self, transactions: &[Transaction]) -> Option<String> {
        let mut total_priority_fees = 0u64;

        if let Some(base_fee_str) = &self.base_fee_per_gas {
            if let Ok(base_fee) = base_fee_str.parse::<u64>() {
                for tx in transactions {
                    if let Ok(gas_price) = tx.gas_price.parse::<u64>() {
                        if gas_price > base_fee {
                            let priority_fee = gas_price - base_fee;
                            total_priority_fees += priority_fee * tx.gas_used as u64;
                        }
                    }
                }
            }
        } else {
            // Pre-EIP-1559: all fees are priority fees
            for tx in transactions {
                if let Ok(gas_price) = tx.gas_price.parse::<u64>() {
                    total_priority_fees += gas_price * tx.gas_used as u64;
                }
            }
        }

        Some(total_priority_fees.to_string())
    }
}

/// Withdrawal data structure (EIP-4895 - Beacon chain push withdrawals)
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Withdrawal {
    #[sqlx(default)]
    pub id: Option<i64>,
    pub block_number: i64,
    pub withdrawal_index: i64,
    pub validator_index: i64,
    pub address: String,
    pub amount: String,
    pub created_at: Option<String>,
}
