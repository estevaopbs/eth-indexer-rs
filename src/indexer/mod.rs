mod block_processor;
mod transaction_processor;

use crate::{
    beacon::BeaconClient, 
    config::AppConfig, 
    database::DatabaseService, 
    rpc::RpcClient,
    token_service::TokenService,
};
use anyhow::Result;
use std::sync::{
    atomic::{AtomicBool, AtomicI64, Ordering},
    Arc,
};
use tokio::sync::mpsc;
use tokio::time::{self, Duration};
use tracing::{error, info, warn, debug};

use block_processor::BlockProcessor;
use transaction_processor::TransactionProcessor;

/// Service for indexing blockchain data with continuous block fetching
pub struct IndexerService {
    db: Arc<DatabaseService>,
    rpc: Arc<RpcClient>,
    beacon: Arc<BeaconClient>,
    config: AppConfig,
    is_running: Arc<AtomicBool>,
    block_processor: BlockProcessor,
    tx_processor: TransactionProcessor,
    // Shared state for the block queue
    next_block_to_fetch: Arc<AtomicI64>,
    latest_network_block: Arc<AtomicI64>,
}

impl IndexerService {
    /// Create a new indexer service with continuous block fetching architecture
    pub fn new(
        db: Arc<DatabaseService>,
        rpc: Arc<RpcClient>,
        beacon: Arc<BeaconClient>,
        config: AppConfig,
    ) -> Self {
        let tx_processor = TransactionProcessor::new(db.clone(), rpc.clone(), config.clone());
        let block_processor = BlockProcessor::new(db.clone(), rpc.clone(), beacon.clone(), tx_processor.clone());

        Self {
            db,
            rpc,
            beacon,
            config,
            is_running: Arc::new(AtomicBool::new(false)),
            block_processor,
            tx_processor,
            next_block_to_fetch: Arc::new(AtomicI64::new(0)),
            latest_network_block: Arc::new(AtomicI64::new(0)),
        }
    }

    /// Create a new indexer service with token service support
    pub fn with_token_service(
        db: Arc<DatabaseService>,
        rpc: Arc<RpcClient>,
        beacon: Arc<BeaconClient>,
        token_service: Arc<TokenService>,
        config: AppConfig,
    ) -> Self {
        let tx_processor = TransactionProcessor::with_token_service(
            db.clone(), 
            rpc.clone(), 
            config.clone(),
            token_service
        );
        let block_processor = BlockProcessor::new(db.clone(), rpc.clone(), beacon.clone(), tx_processor.clone());

        Self {
            db,
            rpc,
            beacon,
            config,
            is_running: Arc::new(AtomicBool::new(false)),
            block_processor,
            tx_processor,
            next_block_to_fetch: Arc::new(AtomicI64::new(0)),
            latest_network_block: Arc::new(AtomicI64::new(0)),
        }
    }

    /// Start the indexer service with continuous block fetching
    pub async fn start_service(&self) -> Result<()> {
        if self.is_running.load(Ordering::Relaxed) {
            warn!("Indexer is already running");
            return Ok(());
        }

        self.is_running.store(true, Ordering::Relaxed);
        info!("Starting indexer service with continuous block fetching");

        // Check RPC connection
        match self.rpc.check_connection().await {
            Ok(true) => {
                info!("RPC connection successful, starting continuous block indexing");

                // Initialize starting block
                self.initialize_start_block().await?;

                // Create block queue channel
                let queue_size = self.config.worker_pool_size * self.config.block_queue_size_multiplier;
                let (block_sender, block_receiver) = mpsc::channel::<i64>(queue_size);
                let receiver = Arc::new(tokio::sync::Mutex::new(block_receiver));

                // Start the block fetcher task (independent loop)
                let fetcher_handle = self.start_block_fetcher(block_sender.clone());

                // Start worker tasks for processing blocks
                let worker_handles = self.start_worker_pool(receiver).await;

                // Wait for either fetcher or workers to complete (they shouldn't unless error)
                tokio::select! {
                    result = fetcher_handle => {
                        error!("Block fetcher stopped unexpectedly: {:?}", result);
                    }
                    _ = async {
                        for handle in worker_handles {
                            if let Err(e) = handle.await {
                                error!("Worker failed: {}", e);
                            }
                        }
                    } => {
                        error!("All workers stopped unexpectedly");
                    }
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

    /// Initialize the starting block based on database state and configuration
    async fn initialize_start_block(&self) -> Result<()> {
        let latest_indexed_block = match self.db.get_latest_block_number().await? {
            Some(num) => {
                info!("Found existing blocks, resuming from block: {}", num + 1);
                num + 1
            }
            None => {
                let start_block = self.config.start_block.map(|n| n as i64).unwrap_or(0);
                info!("No blocks found, starting from configured block: {}", start_block);
                start_block
            }
        };

        self.next_block_to_fetch.store(latest_indexed_block, Ordering::Relaxed);
        
        // Get initial network block number
        let network_block = self.rpc.get_latest_block_number().await? as i64;
        self.latest_network_block.store(network_block, Ordering::Relaxed);
        
        info!("Indexer initialized: next_block={}, network_block={}", latest_indexed_block, network_block);
        Ok(())
    }

    /// Start the independent block fetcher task
    fn start_block_fetcher(&self, block_sender: mpsc::Sender<i64>) -> tokio::task::JoinHandle<()> {
        let rpc = self.rpc.clone();
        let is_running = self.is_running.clone();
        let next_block_to_fetch = self.next_block_to_fetch.clone();
        let latest_network_block = self.latest_network_block.clone();
        let poll_interval = Duration::from_secs(
            self.config.block_fetch_interval_seconds.unwrap_or(3) as u64
        );

        tokio::spawn(async move {
            info!("Block fetcher started with poll interval: {:?}", poll_interval);
            
            while is_running.load(Ordering::Relaxed) {
                match Self::fetch_and_queue_blocks(&rpc, &block_sender, &next_block_to_fetch, &latest_network_block).await {
                    Ok(blocks_queued) => {
                        if blocks_queued > 0 {
                            debug!("Fetcher queued {} new blocks", blocks_queued);
                        }
                    }
                    Err(e) => {
                        error!("Block fetcher error: {}", e);
                    }
                }

                // Wait for next poll cycle
                time::sleep(poll_interval).await;
            }
            
            info!("Block fetcher stopped");
        })
    }

    /// Fetch new blocks from the network and queue them for processing
    async fn fetch_and_queue_blocks(
        rpc: &RpcClient,
        sender: &mpsc::Sender<i64>,
        next_block_to_fetch: &AtomicI64,
        latest_network_block: &AtomicI64,
    ) -> Result<usize> {
        // Get latest network block
        let current_network_block = rpc.get_latest_block_number().await? as i64;
        latest_network_block.store(current_network_block, Ordering::Relaxed);

        let next_block = next_block_to_fetch.load(Ordering::Relaxed);
        
        if next_block > current_network_block {
            // We're ahead of the network, nothing to do
            return Ok(0);
        }

        let mut blocks_queued = 0;
        let mut block_to_queue = next_block;

        // Queue all available blocks up to the current network block
        while block_to_queue <= current_network_block {
            match sender.try_send(block_to_queue) {
                Ok(_) => {
                    debug!("Fetcher queued block #{}", block_to_queue);
                    block_to_queue += 1;
                    blocks_queued += 1;
                }
                Err(mpsc::error::TrySendError::Full(_)) => {
                    // Queue is full, stop queuing for now
                    debug!("Block queue is full, will retry on next cycle");
                    break;
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    // Receiver is closed, workers stopped
                    warn!("Block queue receiver closed, stopping fetcher");
                    break;
                }
            }
        }

        // Update the next block to fetch
        next_block_to_fetch.store(block_to_queue, Ordering::Relaxed);

        if blocks_queued > 0 {
            info!("Queued {} blocks (range: {} to {}), network at block {}", 
                  blocks_queued, next_block, block_to_queue - 1, current_network_block);
        }

        Ok(blocks_queued)
    }

    /// Start the worker pool for processing blocks
    async fn start_worker_pool(
        &self,
        receiver: Arc<tokio::sync::Mutex<mpsc::Receiver<i64>>>,
    ) -> Vec<tokio::task::JoinHandle<()>> {
        let worker_count = self.config.worker_pool_size;
        let mut worker_handles = Vec::new();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(self.config.max_concurrent_blocks));

        info!("Starting {} workers for block processing", worker_count);

        for worker_id in 0..worker_count {
            let receiver_clone = receiver.clone();
            let block_processor = self.block_processor.clone();
            let semaphore_clone = semaphore.clone();
            let is_running = self.is_running.clone();

            let worker_handle = tokio::spawn(async move {
                info!("Worker {} started and ready for blocks", worker_id);

                while is_running.load(Ordering::Relaxed) {
                    // Get next block from queue
                    let block_number = {
                        let mut rx = receiver_clone.lock().await;
                        match time::timeout(Duration::from_secs(10), rx.recv()).await {
                            Ok(Some(block)) => block,
                            Ok(None) => {
                                info!("Worker {} received shutdown signal (channel closed)", worker_id);
                                break;
                            }
                            Err(_) => {
                                // Timeout - no blocks available, continue waiting
                                debug!("Worker {} timeout waiting for blocks", worker_id);
                                continue;
                            }
                        }
                    };

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

        worker_handles
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

    /// Get indexing status for monitoring
    pub fn get_status(&self) -> IndexerStatus {
        IndexerStatus {
            is_running: self.is_running.load(Ordering::Relaxed),
            next_block_to_fetch: self.next_block_to_fetch.load(Ordering::Relaxed),
            latest_network_block: self.latest_network_block.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug)]
pub struct IndexerStatus {
    pub is_running: bool,
    pub next_block_to_fetch: i64,
    pub latest_network_block: i64,
}
