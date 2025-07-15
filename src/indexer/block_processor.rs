use crate::{
    beacon::BeaconClient,
    database::{Block, DatabaseService, Withdrawal},
    rpc::RpcClient,
};
use anyhow::{Context, Result};
use ethers::core::types::{Block as EthBlock, Transaction as EthTransaction};
use std::sync::Arc;
use tracing::{debug, error, info};

use super::transaction_processor::TransactionProcessor;

/// Processor for handling block data
#[derive(Clone)]
pub struct BlockProcessor {
    db: Arc<DatabaseService>,
    rpc: Arc<RpcClient>,
    beacon: Arc<BeaconClient>,          // Now mandatory
    tx_processor: TransactionProcessor, // Shared transaction processor
}

impl BlockProcessor {
    /// Create a new block processor with mandatory Beacon Chain support
    pub fn new(
        db: Arc<DatabaseService>,
        rpc: Arc<RpcClient>,
        beacon: Arc<BeaconClient>,
        tx_processor: TransactionProcessor,
    ) -> Self {
        Self {
            db,
            rpc,
            beacon,
            tx_processor,
        }
    }

    pub async fn process_block(&self, block_number: u64) -> Result<()> {
        let start_time = std::time::Instant::now();

        let block_fetch_start = std::time::Instant::now();
        let eth_block = self
            .rpc
            .get_block_by_number(block_number)
            .await?
            .context(format!("Block #{} not found", block_number))?;
        let block_fetch_time = block_fetch_start.elapsed();

        // Convert to our Block model and save
        let block = self.convert_block(&eth_block).await?;

        let block_insert_start = std::time::Instant::now();
        self.db.insert_block(&block).await?;
        let block_insert_time = block_insert_start.elapsed();

        debug!(
            "Block #{} insert time: {}ms",
            block_number,
            block_insert_time.as_millis()
        );

        // Process withdrawals if present (Shanghai fork)
        if let Some(withdrawals) = &eth_block.withdrawals {
            let withdrawals_start = std::time::Instant::now();
            for (index, withdrawal) in withdrawals.iter().enumerate() {
                let withdrawal_data = Withdrawal {
                    id: None,
                    block_number: block_number as i64,
                    withdrawal_index: index as i64,
                    validator_index: withdrawal.validator_index.as_u64() as i64,
                    address: format!("{:?}", withdrawal.address),
                    amount: withdrawal.amount.to_string(), // Amount in Gwei
                    created_at: None,
                };

                if let Err(e) = self.db.insert_withdrawal(&withdrawal_data).await {
                    error!("Failed to insert withdrawal {}: {}", index, e);
                }
            }
            let withdrawals_time = withdrawals_start.elapsed();
            debug!(
                "Block #{} withdrawals processing time: {}ms",
                block_number,
                withdrawals_time.as_millis()
            );
        }

        if !eth_block.transactions.is_empty() {
            let tx_hashes: Vec<String> = eth_block
                .transactions
                .iter()
                .map(|tx| format!("{:?}", tx.hash))
                .collect();

            let receipts_start = std::time::Instant::now();
            let receipts = self
                .tx_processor
                .get_transaction_receipts_batch(&tx_hashes)
                .await?;
            let receipts_time = receipts_start.elapsed();

            let mut tx_receipt_pairs = Vec::new();
            for (tx, receipt) in eth_block.transactions.iter().zip(receipts.iter()) {
                if let Some(receipt) = receipt {
                    tx_receipt_pairs.push((tx.clone(), receipt.clone()));
                }
            }

            // Process entire block's transactions in one optimized batch
            match self
                .tx_processor
                .collect_block_transaction_data(&tx_receipt_pairs)
                .await
            {
                Ok((all_transactions, all_logs, all_token_transfers, all_accounts)) => {
                    debug!(
                        "Block #{} collected data: {} transactions, {} logs, {} token_transfers, {} accounts",
                        block_number,
                        all_transactions.len(),
                        all_logs.len(),
                        all_token_transfers.len(),
                        all_accounts.len()
                    );

                    // Batch insert all data at once for maximum performance
                    let batch_db_start = std::time::Instant::now();

                    if !all_transactions.is_empty() {
                        if let Err(e) = self.db.insert_transactions_batch(&all_transactions).await {
                            error!("Failed to batch insert transactions: {}", e);
                        }
                    }

                    if !all_logs.is_empty() {
                        if let Err(e) = self.db.insert_logs_batch(&all_logs).await {
                            error!("Failed to batch insert logs: {}", e);
                        }
                    }

                    if !all_token_transfers.is_empty() {
                        if let Err(e) = self
                            .db
                            .insert_token_transfers_batch(&all_token_transfers)
                            .await
                        {
                            error!("Failed to batch insert token transfers: {}", e);
                        }

                        // Process token transfers for token discovery and balance updates
                        if let Err(e) = self
                            .tx_processor
                            .process_token_transfers_with_balances(
                                &all_token_transfers,
                                block_number as i64,
                            )
                            .await
                        {
                            error!("Failed to process token transfers for balances: {}", e);
                        }
                    }

                    if !all_accounts.is_empty() {
                        if let Err(e) = self.db.insert_accounts_batch(&all_accounts).await {
                            error!("Failed to batch insert accounts: {}", e);
                        } else {
                            info!(
                                "Successfully inserted {} accounts from block #{}",
                                all_accounts.len(),
                                block_number
                            );
                        }
                    } else {
                        info!("No accounts to insert for block #{}", block_number);
                    }

                    let batch_db_time = batch_db_start.elapsed();

                    info!("Block #{} performance: block_fetch={}ms, receipts_fetch={}ms, batch_db={}ms, total={}ms", 
                          block_number,
                          block_fetch_time.as_millis(),
                          receipts_time.as_millis(),
                          batch_db_time.as_millis(),
                          start_time.elapsed().as_millis());
                }
                Err(e) => {
                    error!(
                        "Failed to process block {} transactions: {}",
                        block_number, e
                    );
                }
            }
        }
        Ok(())
    }

    /// Convert Ethereum block to our Block model
    async fn convert_block(&self, eth_block: &EthBlock<EthTransaction>) -> Result<Block> {
        let gas_used = eth_block.gas_used.as_u64();
        let base_fee = eth_block.base_fee_per_gas.map(|fee| fee.to_string());

        // Count withdrawals if present
        let withdrawal_count = eth_block
            .withdrawals
            .as_ref()
            .map(|w| w.len() as i64)
            .unwrap_or(0);

        let block_number = eth_block.number.context("Block number missing")?.as_u64();

        // Get Beacon Chain data (now always available)
        let beacon_data = match self.beacon.get_beacon_data_for_block(block_number).await {
            Ok(data) => Some(data),
            Err(e) => {
                debug!(
                    "Failed to fetch beacon data for block {}: {}",
                    block_number, e
                );
                None
            }
        };

        let block = Block {
            number: block_number as i64,
            hash: format!("{:?}", eth_block.hash.context("Block hash missing")?),
            parent_hash: format!("{:?}", eth_block.parent_hash),
            timestamp: eth_block.timestamp.as_u64() as i64,
            gas_used: gas_used as i64,
            gas_limit: eth_block.gas_limit.as_u64() as i64,
            transaction_count: eth_block.transactions.len() as i64,
            miner: Some(format!("{:?}", eth_block.author)),
            difficulty: Some(eth_block.difficulty.to_string()),
            size_bytes: eth_block.size.map(|s| s.as_u64() as i64),
            base_fee_per_gas: base_fee,
            extra_data: Some(format!("{:?}", eth_block.extra_data)),
            state_root: Some(format!("{:?}", eth_block.state_root)),
            nonce: eth_block.nonce.map(|n| format!("{:?}", n)),
            withdrawals_root: eth_block.withdrawals_root.map(|wr| format!("{:?}", wr)),
            blob_gas_used: eth_block.blob_gas_used.map(|bgu| bgu.as_u64() as i64),
            excess_blob_gas: eth_block.excess_blob_gas.map(|ebg| ebg.as_u64() as i64),
            withdrawal_count: Some(withdrawal_count),

            // Beacon Chain fields (from separate API)
            slot: beacon_data.as_ref().and_then(|d| d["slot"].as_i64()),
            proposer_index: beacon_data
                .as_ref()
                .and_then(|d| d["proposer_index"].as_i64()),
            epoch: beacon_data.as_ref().and_then(|d| d["epoch"].as_i64()),
            slot_root: beacon_data
                .as_ref()
                .and_then(|d| d["slot_root"].as_str().map(|s| s.to_string())),
            parent_root: beacon_data
                .as_ref()
                .and_then(|d| d["parent_root"].as_str().map(|s| s.to_string())),
            beacon_deposit_count: beacon_data
                .as_ref()
                .and_then(|d| d["beacon_deposit_count"].as_i64()),
            graffiti: beacon_data
                .as_ref()
                .and_then(|d| d["graffiti"].as_str().map(|s| s.to_string())),
            randao_reveal: beacon_data
                .as_ref()
                .and_then(|d| d["randao_reveal"].as_str().map(|s| s.to_string())),
            randao_mix: beacon_data
                .as_ref()
                .and_then(|d| d["randao_mix"].as_str().map(|s| s.to_string())),
        };

        Ok(block)
    }
}
