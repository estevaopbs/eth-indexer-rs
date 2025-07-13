mod block_processor;
mod transaction_processor;

use crate::{beacon::BeaconClient, config::AppConfig, database::DatabaseService, rpc::RpcClient};
use anyhow::{Context, Result};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::time::{self, Duration};
use tracing::{error, info, warn};

use block_processor::BlockProcessor;
use transaction_processor::TransactionProcessor;

/// Service for indexing blockchain data
pub struct IndexerService {
    db: Arc<DatabaseService>,
    rpc: Arc<RpcClient>,
    beacon: Arc<BeaconClient>,
    config: AppConfig,
    is_running: Arc<AtomicBool>,
    block_processor: BlockProcessor,
    tx_processor: TransactionProcessor,
}

impl IndexerService {
    /// Create a new indexer service with mandatory Beacon Chain support
    pub fn new(
        db: Arc<DatabaseService>,
        rpc: Arc<RpcClient>,
        beacon: Arc<BeaconClient>,
        config: AppConfig,
    ) -> Self {
        let block_processor = BlockProcessor::new(db.clone(), rpc.clone(), beacon.clone());
        let tx_processor = TransactionProcessor::new(db.clone(), rpc.clone());

        Self {
            db,
            rpc,
            beacon,
            config,
            is_running: Arc::new(AtomicBool::new(false)),
            block_processor,
            tx_processor,
        }
    }

    /// Start the indexer service (public method that doesn't require mutability)
    pub async fn start_service(&self) -> Result<()> {
        if self.is_running.load(Ordering::Relaxed) {
            warn!("Indexer is already running");
            return Ok(());
        }

        self.is_running.store(true, Ordering::Relaxed);
        info!("Starting indexer service");

        // Check RPC connection
        match self.rpc.check_connection().await {
            Ok(true) => {
                info!("RPC connection successful, starting blockchain indexing");
                // Main indexing loop
                while self.is_running.load(Ordering::Relaxed) {
                    if let Err(e) = self.sync_blocks().await {
                        error!("Error syncing blocks: {}", e);
                        time::sleep(Duration::from_secs(5)).await;
                    }
                    time::sleep(Duration::from_secs(2)).await;
                }
            }
            _ => {
                warn!("Failed to connect to RPC endpoint");
                self.is_running.store(false, Ordering::Relaxed);
                warn!("Indexer stopped due to RPC connection failure");
            }
        }

        Ok(())
    }

    /// Start the indexer service
    pub async fn start(&mut self) -> Result<()> {
        self.start_service().await
    }

    /// Stop the indexer service
    pub fn stop(&self) {
        if self.is_running.load(Ordering::Relaxed) {
            info!("Stopping indexer service");
            self.is_running.store(false, Ordering::Relaxed);
        } else {
            warn!("Indexer is not running");
        }
    }

    /// Get the service status
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    /// Sync blocks from the blockchain
    async fn sync_blocks(&self) -> Result<()> {
        // Get the latest block from the blockchain
        let latest_chain_block = self.rpc.get_latest_block_number().await?;
        info!("Latest chain block: {}", latest_chain_block);

        // Get the latest block we have indexed
        let latest_indexed_block = match self.db.get_latest_block_number().await? {
            Some(num) => {
                info!("Latest indexed block: {}", num);
                num
            }
            None => {
                let start_block = self.config.start_block.map(|n| n as i64).unwrap_or(0) - 1;
                info!("No blocks found, starting from: {}", start_block);
                start_block
            }
        };

        // If we're already in sync, just return
        if latest_indexed_block >= 0 && latest_indexed_block as u64 >= latest_chain_block {
            info!("Already in sync, no new blocks to process");
            return Ok(());
        }

        info!(
            "Syncing blocks: indexed={}, chain={}, remaining={}",
            latest_indexed_block + 1,
            latest_chain_block,
            latest_chain_block as i64 - latest_indexed_block
        );

        // Process blocks in batches
        let start_block = latest_indexed_block + 1;
        let end_block = std::cmp::min(
            start_block + self.config.blocks_per_batch as i64 - 1,
            latest_chain_block as i64,
        );

        for block_number in start_block..=end_block {
            // Process the block
            match self
                .block_processor
                .process_block(block_number as u64)
                .await
            {
                Ok(_) => {
                    info!("Processed block #{}", block_number);
                }
                Err(e) => {
                    error!("Failed to process block #{}: {}", block_number, e);
                    return Err(anyhow::anyhow!(
                        "Failed to process block #{}: {}",
                        block_number,
                        e
                    ));
                }
            }
        }

        Ok(())
    }
}
