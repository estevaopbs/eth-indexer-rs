mod block_processor;
mod transaction_processor;

use crate::{beacon::BeaconClient, config::AppConfig, database::DatabaseService, rpc::RpcClient};
use anyhow::{Context, Result};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::time::{self, Duration};
use tracing::{error, info, warn, debug};

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
        let tx_processor = TransactionProcessor::new(db.clone(), rpc.clone());
        let block_processor = BlockProcessor::new(db.clone(), rpc.clone(), beacon.clone(), tx_processor.clone());

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
                    time::sleep(Duration::from_millis(500)).await; // Reduced from 2 seconds to 500ms
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

    /// Sync blocks from the blockchain with intelligent queue management
    async fn sync_blocks(&self) -> Result<()> {
        // Get the latest block from the blockchain
        let latest_chain_block = self.rpc.get_latest_block_number().await?;
        debug!("Latest chain block: {}", latest_chain_block);

        // Get the latest block we have indexed
        let latest_indexed_block = match self.db.get_latest_block_number().await? {
            Some(num) => {
                debug!("Latest indexed block: {}", num);
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
            debug!("Already in sync, no new blocks to process");
            return Ok(());
        }

        let remaining_blocks = latest_chain_block as i64 - latest_indexed_block;
        info!(
            "Syncing blocks: indexed={}, chain={}, remaining={}",
            latest_indexed_block + 1,
            latest_chain_block,
            remaining_blocks
        );

        // Smart queue management system with buffer
        let worker_pool_size = self.config.blocks_per_batch;
        let queue_buffer_size = worker_pool_size * 3; // 3x buffer to prevent worker starvation
        let semaphore = Arc::new(tokio::sync::Semaphore::new(self.config.max_concurrent_requests));
        
        // Bounded channel with 3 * BLOCKS_PER_BATCH capacity for better buffering
        let (block_sender, block_receiver) = tokio::sync::mpsc::channel::<i64>(queue_buffer_size);
        let receiver = Arc::new(tokio::sync::Mutex::new(block_receiver));
        
        // Shared state for queue management
        let next_block = Arc::new(tokio::sync::Mutex::new(latest_indexed_block + 1));
        let max_block = latest_chain_block as i64;
        
        // Spawn worker tasks that continuously process blocks
        let mut worker_handles = Vec::new();
        for worker_id in 0..worker_pool_size {
            let receiver_clone = receiver.clone();
            let block_processor = self.block_processor.clone();
            let semaphore_clone = semaphore.clone();
            let sender_clone = block_sender.clone();
            let next_block_clone = next_block.clone();
            
            let worker_handle = tokio::spawn(async move {
                info!("Worker {} started and ready for blocks", worker_id);
                
                loop {
                    // Get next block from shared queue
                    let block_number = {
                        let mut rx = receiver_clone.lock().await;
                        match rx.recv().await {
                            Some(block) => block,
                            None => {
                                info!("Worker {} received shutdown signal", worker_id);
                                break; // Channel closed, shutdown
                            }
                        }
                    };
                    
                    // Try to queue the next block immediately to maintain queue buffer
                    // This ensures we always have up to 3 * BLOCKS_PER_BATCH blocks in the queue
                    tokio::spawn({
                        let sender = sender_clone.clone();
                        let next_block = next_block_clone.clone();
                        async move {
                            let mut next = next_block.lock().await;
                            if *next <= max_block {
                                if let Ok(_) = sender.try_send(*next) {
                                    debug!("Auto-queued next block #{}", *next);
                                    *next += 1;
                                }
                            }
                        }
                    });
                    
                    // Acquire processing permit
                    let permit = match semaphore_clone.acquire().await {
                        Ok(permit) => permit,
                        Err(_) => {
                            error!("Worker {} failed to acquire semaphore permit for block #{}", worker_id, block_number);
                            continue;
                        }
                    };
                    
                    debug!("Worker {} processing block #{}", worker_id, block_number);
                    match block_processor.process_block(block_number as u64).await {
                        Ok(_) => {
                            info!("Worker {} ✅ completed block #{}", worker_id, block_number);
                        }
                        Err(e) => {
                            error!("Worker {} ❌ failed to process block #{}: {}", worker_id, block_number, e);
                            // Continue processing other blocks instead of failing entirely
                        }
                    }
                    drop(permit); // Release permit for next block
                }
                info!("Worker {} shutting down", worker_id);
            });
            worker_handles.push(worker_handle);
        }

        // Initial queue population: fill the queue with buffer blocks for better throughput
        info!("Populating initial queue buffer with up to {} blocks", queue_buffer_size);
        {
            let mut next = next_block.lock().await;
            let start_block = *next;
            
            // Fill the entire buffer initially to minimize worker idle time
            for _ in 0..queue_buffer_size {
                if *next <= max_block {
                    match block_sender.send(*next).await {
                        Ok(_) => {
                            debug!("Initially queued block #{}", *next);
                            *next += 1;
                        }
                        Err(e) => {
                            error!("Failed to initially queue block #{}: {}", *next, e);
                            break;
                        }
                    }
                } else {
                    break;
                }
            }
            info!("Initial queue buffer populated with blocks {} to {}", start_block, *next - 1);
        }

        // Monitor and maintain queue until all blocks are processed
        let monitor_handle = tokio::spawn({
            let sender = block_sender.clone();
            let next_block = next_block.clone();
            async move {
                loop {
                    // Check if we need to add more blocks to maintain queue size
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    
                    let mut next = next_block.lock().await;
                    if *next > max_block {
                        info!("All blocks queued, monitor shutting down");
                        break;
                    }
                    
                    // Try to send next block if queue has space
                    if let Ok(_) = sender.try_send(*next) {
                        debug!("Monitor queued block #{}", *next);
                        *next += 1;
                    }
                    // If try_send fails, queue is full - this is what we want!
                }
                drop(sender); // Signal end of blocks to workers
            }
        });

        // Wait for monitor to finish queuing all blocks
        if let Err(e) = monitor_handle.await {
            error!("Monitor task failed: {}", e);
        }
        
        // Wait for all workers to complete processing
        for (worker_id, handle) in worker_handles.into_iter().enumerate() {
            match handle.await {
                Ok(_) => info!("Worker {} completed successfully", worker_id),
                Err(e) => error!("Worker {} failed: {}", worker_id, e),
            }
        }

        info!("Completed syncing {} blocks with intelligent queue management ({}x buffer)", remaining_blocks, queue_buffer_size / worker_pool_size);
        Ok(())
    }
}
