use crate::config::AppConfig;
use anyhow::{Context, Result};
use ethers::{
    core::types::{
        Block as EthBlock, BlockNumber, Transaction as EthTransaction, TransactionReceipt, H256,
        U64,
    },
    providers::{Http, Middleware, Provider},
};
use std::str::FromStr;
use std::sync::Arc;
use tracing::{debug, error};

/// Client for interacting with Ethereum RPC
pub struct RpcClient {
    provider: Arc<Provider<Http>>,
    config: AppConfig,
}

impl RpcClient {
    /// Create a new RPC client
    pub fn new(rpc_url: &str, config: AppConfig) -> Result<Self> {
        let provider = Provider::<Http>::try_from(rpc_url)
            .context(format!("Failed to connect to RPC URL: {}", rpc_url))?;

        Ok(Self {
            provider: Arc::new(provider),
            config,
        })
    }

    /// Get the latest block number
    pub async fn get_latest_block_number(&self) -> Result<u64> {
        let block_number = self
            .provider
            .get_block_number()
            .await
            .context("Failed to get latest block number")?;

        Ok(block_number.as_u64())
    }

    /// Get block by number
    pub async fn get_block_by_number(
        &self,
        number: u64,
    ) -> Result<Option<EthBlock<EthTransaction>>> {
        let block_number = BlockNumber::Number(number.into());
        let block = self
            .provider
            .get_block_with_txs(block_number)
            .await
            .context(format!("Failed to get block by number: {}", number))?;

        Ok(block)
    }

    /// Get block by hash
    pub async fn get_block_by_hash(&self, hash: &str) -> Result<Option<EthBlock<EthTransaction>>> {
        let hash = H256::from_str(hash).context(format!("Invalid block hash: {}", hash))?;

        let block = self
            .provider
            .get_block_with_txs(hash)
            .await
            .context(format!("Failed to get block by hash: {}", hash))?;

        Ok(block)
    }

    /// Get transaction receipt
    pub async fn get_transaction_receipt(
        &self,
        tx_hash: &str,
    ) -> Result<Option<TransactionReceipt>> {
        let hash =
            H256::from_str(tx_hash).context(format!("Invalid transaction hash: {}", tx_hash))?;

        let receipt = self
            .provider
            .get_transaction_receipt(hash)
            .await
            .context(format!("Failed to get transaction receipt: {}", tx_hash))?;

        Ok(receipt)
    }

    /// Get account balance
    pub async fn get_balance(&self, address: &str, block_number: Option<u64>) -> Result<String> {
        let address = address
            .parse::<ethers::core::types::H160>()
            .context(format!("Invalid Ethereum address: {}", address))?;

        let balance = match block_number {
            Some(num) => {
                self.provider
                    .get_balance(
                        address,
                        Some(ethers::core::types::BlockId::Number(BlockNumber::Number(
                            U64::from(num),
                        ))),
                    )
                    .await
            }
            None => self.provider.get_balance(address, None).await,
        }
        .context(format!("Failed to get balance for address: {}", address))?;

        Ok(balance.to_string())
    }

    /// Check connection to RPC
    pub async fn check_connection(&self) -> Result<bool> {
        match self.provider.get_block_number().await {
            Ok(_) => {
                debug!("Successfully connected to Ethereum RPC");
                Ok(true)
            }
            Err(e) => {
                error!("Failed to connect to Ethereum RPC: {}", e);
                Ok(false)
            }
        }
    }
}
