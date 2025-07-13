use crate::{
    beacon::BeaconClient,
    database::{Block, DatabaseService, Withdrawal},
    rpc::RpcClient,
};
use anyhow::{Context, Result};
use ethers::core::types::{Block as EthBlock, Transaction as EthTransaction};
use std::sync::Arc;
use tracing::{debug, error, warn};

use super::transaction_processor::TransactionProcessor;

/// Processor for handling block data
pub struct BlockProcessor {
    db: Arc<DatabaseService>,
    rpc: Arc<RpcClient>,
    beacon: Arc<BeaconClient>, // Now mandatory
}

impl BlockProcessor {
    /// Create a new block processor with mandatory Beacon Chain support
    pub fn new(db: Arc<DatabaseService>, rpc: Arc<RpcClient>, beacon: Arc<BeaconClient>) -> Self {
        Self { db, rpc, beacon }
    }

    /// Process a block and its transactions
    pub async fn process_block(&self, block_number: u64) -> Result<()> {
        debug!("Processing block #{}", block_number);

        // Get block data from RPC
        let eth_block = self
            .rpc
            .get_block_by_number(block_number)
            .await?
            .context(format!("Block #{} not found", block_number))?;

        // Convert to our Block model and save
        let block = self.convert_block(&eth_block).await?;
        self.db.insert_block(&block).await?;

        // Process withdrawals if present (Shanghai fork)
        if let Some(withdrawals) = &eth_block.withdrawals {
            for (index, withdrawal) in withdrawals.iter().enumerate() {
                let withdrawal_data = Withdrawal {
                    id: None,
                    block_number: block_number as i64,
                    withdrawal_index: index as i64,
                    validator_index: withdrawal.validator_index.as_u64() as i64,
                    address: format!("{:?}", withdrawal.address),
                    amount: withdrawal.amount.to_string(), // Amount in Gwei
                };

                if let Err(e) = self.db.insert_withdrawal(&withdrawal_data).await {
                    error!("Failed to insert withdrawal {}: {}", index, e);
                }
            }
        }

        // Process transactions
        let tx_processor = TransactionProcessor::new(self.db.clone(), self.rpc.clone());

        for tx in eth_block.transactions {
            if let Err(e) = tx_processor.process_transaction(&tx).await {
                error!("Failed to process transaction {}: {}", tx.hash, e);
                // Continue processing other transactions
            }
        }

        debug!("Completed processing block #{}", block_number);
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

            // Extended RPC fields available in ethers 2.0.14
            miner: Some(format!("{:?}", eth_block.author)),
            total_difficulty: eth_block.total_difficulty.map(|td| td.to_string()),
            size_bytes: eth_block.size.map(|s| s.as_u64() as i64),
            base_fee_per_gas: base_fee,
            extra_data: Some(format!("{:?}", eth_block.extra_data)),
            state_root: Some(format!("{:?}", eth_block.state_root)),
            nonce: eth_block.nonce.map(|n| format!("{:?}", n)),

            // New fields available in ethers 2.0.14
            withdrawals_root: eth_block.withdrawals_root.map(|wr| format!("{:?}", wr)),
            blob_gas_used: eth_block.blob_gas_used.map(|bgu| bgu.as_u64() as i64),
            excess_blob_gas: eth_block.excess_blob_gas.map(|ebg| ebg.as_u64() as i64),
            withdrawal_count: Some(withdrawal_count),

            // Beacon Chain fields (from separate API)
            slot: beacon_data.as_ref().and_then(|d| d.slot),
            proposer_index: beacon_data.as_ref().and_then(|d| d.proposer_index),
            epoch: beacon_data.as_ref().and_then(|d| d.epoch),
            slot_root: beacon_data.as_ref().and_then(|d| d.slot_root.clone()),
            parent_root: beacon_data.as_ref().and_then(|d| d.parent_root.clone()),
            beacon_deposit_count: beacon_data.as_ref().and_then(|d| d.beacon_deposit_count),
            graffiti: beacon_data.as_ref().and_then(|d| d.graffiti.clone()),
            randao_reveal: beacon_data.as_ref().and_then(|d| d.randao_reveal.clone()),
            randao_mix: beacon_data.as_ref().and_then(|d| d.randao_mix.clone()),
        };

        Ok(block)
    }
}
