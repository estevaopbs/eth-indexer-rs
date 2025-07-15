use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::collections::HashMap;

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
            if let Ok(base_fee_val) = base_fee_str.parse::<u128>() {
                return Some((base_fee_val * self.gas_used as u128).to_string());
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
    pub base_validator_reward: Option<String>,
    pub mev_reward: Option<String>,
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
            base_validator_reward: None, // Calculated separately with transaction data
            mev_reward: None,    // Calculated separately with transaction data
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
        let beacon_data = self.extract_beacon_data();
        self.calculate_block_reward_with_transactions_and_beacon(
            transactions,
            beacon_data.as_ref(),
        );
    }

    /// Calculate block reward with transaction data and optional beacon chain data
    /// This includes priority fees (tips), base validator reward, and potential MEV
    pub fn calculate_block_reward_with_transactions_and_beacon(
        &mut self,
        transactions: &[Transaction],
        beacon_data: Option<&serde_json::Value>,
    ) {
        let mut total_priority_fees = 0u128;

        if let Some(base_fee_str) = &self.base_fee_per_gas {
            if let Ok(base_fee) = base_fee_str.parse::<u128>() {
                for tx in transactions {
                    if let Ok(gas_price) = tx.gas_price.parse::<u128>() {
                        // Priority fee = gas_price - base_fee (for legacy transactions)
                        // For EIP-1559 transactions, this would be max_priority_fee_per_gas
                        if gas_price > base_fee {
                            let priority_fee = gas_price - base_fee;
                            total_priority_fees += priority_fee * tx.gas_used as u128;
                        }
                    }
                }
            }
        } else {
            // Pre-EIP-1559 blocks: all gas fees go to miner
            for tx in transactions {
                if let Ok(gas_price) = tx.gas_price.parse::<u128>() {
                    total_priority_fees += gas_price * tx.gas_used as u128;
                }
            }
        }

        self.priority_fees = Some(total_priority_fees.to_string());

        // Calculate base validator reward using beacon chain data
        let base_validator_reward = self.calculate_base_validator_reward(beacon_data);
        self.base_validator_reward = Some(base_validator_reward.to_string());

        // Calculate MEV reward (simplified estimation)
        let mev_reward = self.estimate_mev_reward(transactions, total_priority_fees);
        self.mev_reward = Some(mev_reward.to_string());

        // Calculate total block reward
        // In PoS, block reward = base_validator_reward + priority_fees + MEV
        let total_reward = base_validator_reward + total_priority_fees + mev_reward;

        self.block_reward = Some(total_reward.to_string());
    }

    /// Calculate base validator reward using beacon chain data
    /// Uses real Ethereum PoS reward calculation formulas
    fn calculate_base_validator_reward(&self, beacon_data: Option<&serde_json::Value>) -> u128 {
        // Pre-merge blocks don't have validator rewards
        if self.number < 15_537_394 {
            return 0;
        }

        if let Some(beacon) = beacon_data {
            if let Some(slot) = beacon.get("slot").and_then(|s| s.as_u64()) {
                return self.calculate_real_validator_reward(slot);
            }
        }

        // Fallback: Use network average for post-merge blocks
        self.calculate_fallback_validator_reward()
    }

    /// Calculate real validator reward using Ethereum PoS formulas
    fn calculate_real_validator_reward(&self, slot: u64) -> u128 {
        // Real Ethereum PoS reward calculation:
        // base_reward = effective_balance * BASE_REWARD_FACTOR / sqrt(total_active_balance)
        // proposer_reward = base_reward / PROPOSER_REWARD_QUOTIENT

        // Constants from Ethereum specification
        const BASE_REWARD_FACTOR: u128 = 64;
        const PROPOSER_REWARD_QUOTIENT: u128 = 8;
        const MAX_EFFECTIVE_BALANCE: u128 = 32_000_000_000; // 32 ETH in Gwei

        // Get effective balance (assume 32 ETH for full validator)
        let effective_balance = MAX_EFFECTIVE_BALANCE; // 32 ETH in Gwei

        // Estimate total active balance based on network state
        let total_active_balance = self.estimate_total_active_balance(slot);

        // Calculate base reward per epoch
        // base_reward = effective_balance * BASE_REWARD_FACTOR / sqrt(total_active_balance)
        let sqrt_total_balance = (total_active_balance as f64).sqrt() as u128;
        let base_reward_per_epoch = if sqrt_total_balance > 0 {
            (effective_balance * BASE_REWARD_FACTOR) / sqrt_total_balance
        } else {
            0
        };

        // Calculate proposer reward
        // Proposer gets 1/8 of the base reward per included attestation
        // Assume average of 128 attestations per block (4 committees * 32 slots)
        let expected_attestations = 128u128;
        let proposer_reward =
            (base_reward_per_epoch * expected_attestations) / PROPOSER_REWARD_QUOTIENT;

        // Apply inclusion rewards and sync committee rewards if applicable
        let mut total_reward = proposer_reward;

        // Add sync committee reward if proposer is in sync committee
        if self.is_sync_committee_period(slot) {
            let sync_reward = base_reward_per_epoch / 4; // ~25% bonus for sync committee
            total_reward += sync_reward;
        }

        // Add attestation inclusion rewards (validators get rewarded for including attestations)
        let inclusion_reward = (base_reward_per_epoch * expected_attestations) / 64; // Small reward per inclusion
        total_reward += inclusion_reward;

        // Convert from Gwei to Wei
        total_reward * 1_000_000_000
    }

    /// Estimate total active balance on the network
    fn estimate_total_active_balance(&self, slot: u64) -> u128 {
        // Estimate based on historical network growth
        // This is more accurate than a fixed constant

        let epoch = slot / 32;

        // Network started with ~524k validators at merge (~16.8M ETH staked)
        // Growth rate has been approximately 2-3% per month
        const INITIAL_STAKED_ETH_GWEI: u128 = 16_800_000 * 1_000_000_000; // 16.8M ETH in Gwei
        const MERGE_EPOCH: u64 = 144896; // Approximate merge epoch

        if epoch <= MERGE_EPOCH {
            return INITIAL_STAKED_ETH_GWEI;
        }

        // Calculate months since merge (assuming ~7200 epochs per month)
        let epochs_since_merge = epoch - MERGE_EPOCH;
        let months_since_merge = epochs_since_merge / 7200;

        // Apply growth rate (2.5% per month average)
        let growth_factor = (1.025_f64).powf(months_since_merge as f64);
        let current_staked_gwei = (INITIAL_STAKED_ETH_GWEI as f64 * growth_factor) as u128;

        // Cap at reasonable maximum (e.g., 40M ETH = ~1.25M validators)
        const MAX_STAKED_ETH_GWEI: u128 = 40_000_000 * 1_000_000_000;
        current_staked_gwei.min(MAX_STAKED_ETH_GWEI)
    }

    /// Check if the slot is in a sync committee period
    fn is_sync_committee_period(&self, slot: u64) -> bool {
        // Sync committee changes every 256 epochs (8192 slots)
        // Assume 1/512 chance of being in sync committee (512 validators per sync committee)
        let sync_period = slot / 8192;

        // Use proposer index if available for more accuracy
        if let Some(proposer_index) = self.proposer_index {
            // Simple heuristic: check if proposer index aligns with sync committee
            (proposer_index as u64 + sync_period) % 512 == 0
        } else {
            // Default to no sync committee reward
            false
        }
    }

    /// Calculate fallback validator reward when beacon data is unavailable
    fn calculate_fallback_validator_reward(&self) -> u128 {
        // Use time-based estimation for more accuracy
        let block_timestamp = self.timestamp;

        // The Merge timestamp: September 15, 2022, 06:42:42 UTC
        const MERGE_TIMESTAMP: i64 = 1663224162;

        if block_timestamp < MERGE_TIMESTAMP {
            return 0;
        }

        // Calculate months since merge for growth estimation
        let seconds_since_merge = block_timestamp - MERGE_TIMESTAMP;
        let months_since_merge = seconds_since_merge / (30 * 24 * 60 * 60); // Approximate

        // Base reward decreases as more validators join (due to sqrt in denominator)
        // Start with ~0.05 ETH per block proposal, decreasing over time
        let initial_reward_wei: u128 = 50_000_000_000_000_000; // 0.05 ETH
        let decay_factor = 0.98_f64.powf(months_since_merge as f64); // 2% decay per month

        (initial_reward_wei as f64 * decay_factor).max(10_000_000_000_000_000.0) as u128
        // Min 0.01 ETH
    }

    /// Estimate MEV (Maximum Extractable Value) reward
    /// Enhanced analysis of transaction patterns for more accurate MEV detection
    fn estimate_mev_reward(&self, transactions: &[Transaction], priority_fees: u128) -> u128 {
        if transactions.is_empty() {
            return 0;
        }

        let mut mev_indicators = MevAnalysis::new();

        // Analyze transaction patterns for MEV indicators
        self.analyze_transaction_patterns(transactions, &mut mev_indicators);

        // Calculate MEV based on different strategies
        let arbitrage_mev = self.calculate_arbitrage_mev(&mev_indicators, priority_fees);
        let sandwich_mev = self.calculate_sandwich_mev(&mev_indicators);
        let liquidation_mev = self.calculate_liquidation_mev(&mev_indicators);
        let frontrunning_mev = self.calculate_frontrunning_mev(&mev_indicators);

        arbitrage_mev + sandwich_mev + liquidation_mev + frontrunning_mev
    }

    /// Analyze transaction patterns to identify MEV opportunities
    fn analyze_transaction_patterns(
        &self,
        transactions: &[Transaction],
        analysis: &mut MevAnalysis,
    ) {
        let base_fee = self
            .base_fee_per_gas
            .as_ref()
            .and_then(|fee| fee.parse::<u128>().ok())
            .unwrap_or(0);

        for (i, tx) in transactions.iter().enumerate() {
            let gas_price = tx.gas_price.parse::<u128>().unwrap_or(0);
            let priority_fee = if gas_price > base_fee {
                gas_price - base_fee
            } else {
                0
            };
            let value = tx.value.parse::<u128>().unwrap_or(0);

            // High priority fee transactions (potential MEV)
            if priority_fee > base_fee * 20 {
                // 20x base fee threshold
                analysis.high_priority_txs.push(i);
            }

            // Large value transactions (potential targets)
            if value > 1_000_000_000_000_000_000 {
                // > 1 ETH
                analysis.high_value_txs.push(i);
            }

            // Check for DEX/DeFi contract interactions
            if let Some(to_addr) = &tx.to_address {
                if self.is_dex_contract(to_addr) {
                    analysis.dex_interactions.push(i);
                }
                if self.is_lending_contract(to_addr) {
                    analysis.lending_interactions.push(i);
                }
            }

            // Detect potential sandwich patterns (high-low-high gas prices)
            if i > 0 && i < transactions.len() - 1 {
                let prev_gas = transactions[i - 1].gas_price.parse::<u128>().unwrap_or(0);
                let next_gas = transactions[i + 1].gas_price.parse::<u128>().unwrap_or(0);

                if gas_price < prev_gas * 50 / 100 && gas_price < next_gas * 50 / 100 {
                    analysis.sandwich_victims.push(i);
                }
            }

            // Flash loan patterns (same address multiple large transactions)
            if value > 10_000_000_000_000_000_000 {
                // > 10 ETH
                *analysis
                    .flash_loan_candidates
                    .entry(tx.from_address.clone())
                    .or_insert(0) += 1;
            }
        }

        analysis.total_transactions = transactions.len();
    }

    /// Calculate MEV from arbitrage opportunities
    fn calculate_arbitrage_mev(&self, analysis: &MevAnalysis, priority_fees: u128) -> u128 {
        // Arbitrage MEV is typically captured through high priority fee transactions to DEX contracts
        let arbitrage_txs: Vec<_> = analysis
            .high_priority_txs
            .iter()
            .filter(|&&i| analysis.dex_interactions.contains(&i))
            .collect();

        if arbitrage_txs.is_empty() {
            return 0;
        }

        // Estimate: 30-50% of excessive priority fees from DEX arbitrage transactions
        let arbitrage_ratio = (arbitrage_txs.len() as f64) / (analysis.total_transactions as f64);
        if arbitrage_ratio > 0.05 {
            // > 5% of transactions are potential arbitrage
            (priority_fees as f64 * 0.4 * arbitrage_ratio) as u128
        } else {
            0
        }
    }

    /// Calculate MEV from sandwich attacks
    fn calculate_sandwich_mev(&self, analysis: &MevAnalysis) -> u128 {
        if analysis.sandwich_victims.is_empty() {
            return 0;
        }

        // Sandwich attacks typically extract 0.1-1% of victim transaction value
        // Estimate based on number of potential victims and average transaction value
        let sandwich_count = analysis.sandwich_victims.len() as u128;
        let estimated_victim_value: u128 = 5_000_000_000_000_000_000; // Assume 5 ETH average

        // Conservative estimate: 0.2% extraction per sandwich
        (sandwich_count * estimated_victim_value * 2) / 1000
    }

    /// Calculate MEV from liquidation opportunities
    fn calculate_liquidation_mev(&self, analysis: &MevAnalysis) -> u128 {
        // Liquidations typically happen on lending protocols
        let liquidation_txs: Vec<_> = analysis
            .high_priority_txs
            .iter()
            .filter(|&&i| analysis.lending_interactions.contains(&i))
            .collect();

        if liquidation_txs.is_empty() {
            return 0;
        }

        // Liquidation MEV is typically 5-15% of liquidated amount
        // Estimate based on lending protocol interactions with high priority fees
        let liquidation_count = liquidation_txs.len() as u128;
        let estimated_liquidation_value: u128 = 5_000_000_000_000_000_000; // Assume 5 ETH average liquidation

        // Conservative estimate: 8% MEV per liquidation
        (liquidation_count * estimated_liquidation_value * 8) / 100
    }

    /// Calculate MEV from frontrunning
    fn calculate_frontrunning_mev(&self, analysis: &MevAnalysis) -> u128 {
        // Frontrunning is harder to detect but correlates with flash loan usage
        let flash_loan_users = analysis.flash_loan_candidates.len() as u128;

        if flash_loan_users == 0 {
            return 0;
        }

        // Estimate frontrunning MEV based on flash loan activity
        // Typically 1-3 ETH per sophisticated MEV operation
        flash_loan_users * 2_000_000_000_000_000_000 // 2 ETH per operation
    }

    /// Check if address is a known DEX contract
    fn is_dex_contract(&self, address: &str) -> bool {
        // Known DEX contract addresses (Uniswap, SushiSwap, 1inch, etc.)
        const DEX_CONTRACTS: &[&str] = &[
            "0x7a250d5630b4cf539739df2c5dacb4c659f2488d", // Uniswap V2 Router
            "0xe592427a0aece92de3edee1f18e0157c05861564", // Uniswap V3 Router
            "0xd9e1ce17f2641f24ae83637ab66a2cca9c378b9f", // SushiSwap Router
            "0x1111111254fb6c44bac0bed2854e76f90643097d", // 1inch V4 Router
            "0x11111112542d85b3ef69ae05771c2dccff4faa26", // 1inch V3 Router
            "0xdef171fe48cf0115b1d80b88dc8eab59176fee57", // ParaSwap Router
        ];

        let addr_lower = address.to_lowercase();
        DEX_CONTRACTS.iter().any(|&dex| dex == addr_lower)
    }

    /// Check if address is a known lending protocol contract
    fn is_lending_contract(&self, address: &str) -> bool {
        // Known lending protocol addresses (Aave, Compound, MakerDAO, etc.)
        const LENDING_CONTRACTS: &[&str] = &[
            "0x7d2768de32b0b80b7a3454c06bdac94a69ddc7a9", // Aave V2 Pool
            "0x87870bca3f3fd6335c3f4ce8392d69350b4fa4e2", // Aave V3 Pool
            "0x3d9819210a31b4961b30ef54be2aed79b9c9cd3b", // Compound cDAI
            "0x35a18000230da775cac24873d00ff85bccded550", // cUNI
            "0x9759a6ac90977b93b58547b4a71c78317f391a28", // MakerDAO PSM
        ];

        let addr_lower = address.to_lowercase();
        LENDING_CONTRACTS
            .iter()
            .any(|&lending| lending == addr_lower)
    }

    /// Calculate priority fees (tips) from transactions
    pub fn calculate_priority_fees(&self, transactions: &[Transaction]) -> Option<String> {
        let mut total_priority_fees = 0u128;

        if let Some(base_fee_str) = &self.base_fee_per_gas {
            if let Ok(base_fee) = base_fee_str.parse::<u128>() {
                for tx in transactions {
                    if let Ok(gas_price) = tx.gas_price.parse::<u128>() {
                        if gas_price > base_fee {
                            let priority_fee = gas_price - base_fee;
                            total_priority_fees += priority_fee * tx.gas_used as u128;
                        }
                    }
                }
            }
        } else {
            // Pre-EIP-1559: all fees are priority fees
            for tx in transactions {
                if let Ok(gas_price) = tx.gas_price.parse::<u128>() {
                    total_priority_fees += gas_price * tx.gas_used as u128;
                }
            }
        }

        Some(total_priority_fees.to_string())
    }

    /// Extract beacon chain data from block for reward calculations
    fn extract_beacon_data(&self) -> Option<serde_json::Value> {
        // Only create beacon data if we have at least slot information
        if let Some(slot) = self.slot {
            Some(serde_json::json!({
                "slot": slot,
                "proposer_index": self.proposer_index,
                "epoch": self.epoch,
                "slot_root": self.slot_root,
                "parent_root": self.parent_root,
                "beacon_deposit_count": self.beacon_deposit_count,
                "graffiti": self.graffiti,
                "randao_reveal": self.randao_reveal,
                "randao_mix": self.randao_mix
            }))
        } else {
            None
        }
    }

    /// Convert Wei to ETH with high precision
    fn wei_to_eth_string(wei: u128, decimal_places: u32) -> String {
        const WEI_PER_ETH: u128 = 1_000_000_000_000_000_000;
        let eth_whole = wei / WEI_PER_ETH;
        let wei_remainder = wei % WEI_PER_ETH;

        if decimal_places == 0 {
            return eth_whole.to_string();
        }

        let scale = 10_u128.pow(decimal_places);
        let fraction = (wei_remainder * scale) / WEI_PER_ETH;

        format!(
            "{}.{:0width$}",
            eth_whole,
            fraction,
            width = decimal_places as usize
        )
    }

    /// Calculate effective validator reward rate (APR)
    pub fn calculate_validator_apr(&self) -> Option<f64> {
        if let Some(reward_str) = &self.base_validator_reward {
            if let Ok(reward_wei) = reward_str.parse::<u128>() {
                // Assume 32 ETH staked per validator
                const VALIDATOR_STAKE_WEI: u128 = 32 * 1_000_000_000_000_000_000;

                // Calculate annual reward (assuming one block every 12 seconds)
                const SECONDS_PER_YEAR: u128 = 365 * 24 * 60 * 60;
                const SECONDS_PER_BLOCK: u128 = 12;
                const BLOCKS_PER_YEAR: u128 = SECONDS_PER_YEAR / SECONDS_PER_BLOCK;

                let annual_reward = reward_wei * BLOCKS_PER_YEAR;
                let apr = (annual_reward as f64) / (VALIDATOR_STAKE_WEI as f64);

                return Some(apr * 100.0); // Convert to percentage
            }
        }
        None
    }

    /// Get formatted reward breakdown for display
    pub fn get_reward_breakdown(&self) -> serde_json::Value {
        serde_json::json!({
            "total_reward": {
                "wei": self.block_reward.clone().unwrap_or_else(|| "0".to_string()),
                "eth": self.block_reward.as_ref()
                    .and_then(|r| r.parse::<u128>().ok())
                    .map(|wei| Self::wei_to_eth_string(wei, 6))
                    .unwrap_or_else(|| "0.0".to_string())
            },
            "base_validator_reward": {
                "wei": self.base_validator_reward.clone().unwrap_or_else(|| "0".to_string()),
                "eth": self.base_validator_reward.as_ref()
                    .and_then(|r| r.parse::<u128>().ok())
                    .map(|wei| Self::wei_to_eth_string(wei, 6))
                    .unwrap_or_else(|| "0.0".to_string())
            },
            "priority_fees": {
                "wei": self.priority_fees.clone().unwrap_or_else(|| "0".to_string()),
                "eth": self.priority_fees.as_ref()
                    .and_then(|r| r.parse::<u128>().ok())
                    .map(|wei| Self::wei_to_eth_string(wei, 6))
                    .unwrap_or_else(|| "0.0".to_string())
            },
            "mev_reward": {
                "wei": self.mev_reward.clone().unwrap_or_else(|| "0".to_string()),
                "eth": self.mev_reward.as_ref()
                    .and_then(|r| r.parse::<u128>().ok())
                    .map(|wei| Self::wei_to_eth_string(wei, 6))
                    .unwrap_or_else(|| "0.0".to_string())
            },
            "burnt_fees": {
                "wei": self.burnt_fees.clone().unwrap_or_else(|| "0".to_string()),
                "eth": self.burnt_fees.as_ref()
                    .and_then(|r| r.parse::<u128>().ok())
                    .map(|wei| Self::wei_to_eth_string(wei, 6))
                    .unwrap_or_else(|| "0.0".to_string())
            },
            "validator_apr": self.calculate_validator_apr()
        })
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

/// MEV analysis helper structure
#[derive(Debug, Default)]
struct MevAnalysis {
    pub high_priority_txs: Vec<usize>,
    pub high_value_txs: Vec<usize>,
    pub dex_interactions: Vec<usize>,
    pub lending_interactions: Vec<usize>,
    pub sandwich_victims: Vec<usize>,
    pub flash_loan_candidates: HashMap<String, u32>,
    pub total_transactions: usize,
}

impl MevAnalysis {
    fn new() -> Self {
        Self::default()
    }
}
