use anyhow::Result;
use std::{
    sync::Arc,
    time::{Duration},
};
use tokio::{
    sync::{mpsc, oneshot, Semaphore},
    time,
};
use tracing::{debug, error};

/// Request wrapper for the RPC executor
pub struct RpcRequest<T, R> {
    pub operation: T,
    pub response_sender: oneshot::Sender<Result<R>>,
}

/// RPC Executor with rate limiting and concurrency control
pub struct RpcExecutor<T, R>
where
    T: Send + 'static,
    R: Send + 'static,
{
    request_sender: mpsc::UnboundedSender<RpcRequest<T, R>>,
    _handle: tokio::task::JoinHandle<()>,
}

impl<T, R> RpcExecutor<T, R>
where
    T: Send + 'static,
    R: Send + 'static,
{
    /// Create a new RPC executor with rate limiting
    pub fn new<F, Fut>(
        name: String,
        max_concurrent: usize,
        min_interval_ms: u64,
        executor_fn: F,
    ) -> Self
    where
        F: Fn(T) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<R>> + Send + 'static,
    {
        let (request_sender, mut request_receiver) = mpsc::unbounded_channel::<RpcRequest<T, R>>();
        let executor_fn = Arc::new(executor_fn);
        let semaphore = Arc::new(Semaphore::new(max_concurrent));
        let min_interval = Duration::from_millis(min_interval_ms);

        debug!(
            "{} RPC Executor starting: max_concurrent={}, min_interval={}ms",
            name, max_concurrent, min_interval_ms
        );

        let handle = tokio::spawn(async move {
            while let Some(request) = request_receiver.recv().await {
                let executor_fn = executor_fn.clone();
                let semaphore = semaphore.clone();
                let request_name = name.clone();

                // Spawn task to handle the request with concurrency control and rate limiting
                tokio::spawn(async move {
                    // Acquire semaphore permit for concurrency control
                    let _permit = match semaphore.acquire().await {
                        Ok(permit) => permit,
                        Err(_) => {
                            error!("{} RPC failed to acquire semaphore permit", request_name);
                            let _ = request
                                .response_sender
                                .send(Err(anyhow::anyhow!("Failed to acquire semaphore permit")));
                            return;
                        }
                    };

                    // Rate limiting per request (after acquiring permit)
                    if min_interval > Duration::ZERO {
                        time::sleep(min_interval).await;
                    }

                    debug!("{} RPC executing request", request_name);

                    // Execute the request
                    let result = executor_fn(request.operation).await;

                    // Send response back
                    if let Err(_) = request.response_sender.send(result) {
                        debug!("{} RPC response receiver dropped", request_name);
                    }
                });
            }

            debug!("{} RPC Executor stopped", name);
        });

        Self {
            request_sender,
            _handle: handle,
        }
    }

    /// Execute a request through the rate-limited executor
    pub async fn execute(&self, operation: T) -> Result<R> {
        let (response_sender, response_receiver) = oneshot::channel();

        let request = RpcRequest {
            operation,
            response_sender,
        };

        // Send request to executor
        self.request_sender
            .send(request)
            .map_err(|_| anyhow::anyhow!("RPC executor receiver dropped"))?;

        // Wait for response
        response_receiver
            .await
            .map_err(|_| anyhow::anyhow!("RPC request response sender dropped"))?
    }
}

/// Enum for ETH RPC operations
#[derive(Debug)]
pub enum EthRpcOperation {
    GetLatestBlockNumber,
    GetBlockByNumber(u64),
    GetTransactionReceipt(String),
    CheckConnection,
}

/// Enum for Beacon RPC operations  
#[derive(Debug, Clone)]
pub enum BeaconRpcOperation {
    TestConnection,
    GetBlockHeader(u64),
    GetBlock(u64),
    GetSlotByExecutionBlock(u64),
    GetDepositCount,
    GetBeaconDataForBlock(u64),
}

/// Response types for Beacon RPC operations
#[derive(Debug, Clone)]
pub enum BeaconRpcResponse {
    TestConnection(()),
    BlockHeader(Option<serde_json::Value>),
    Block(Option<serde_json::Value>),
    SlotByExecutionBlock(Option<u64>),
    DepositCount(u64),
    BeaconDataForBlock(serde_json::Value),
}
