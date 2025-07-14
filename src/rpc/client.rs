use crate::config::AppConfig;
use anyhow::{Context, Result};
use ethers::{
    core::types::{
        Block as EthBlock, BlockNumber, Transaction as EthTransaction, TransactionReceipt, H256,
        U64, H160, Bytes, TransactionRequest,
    },
    providers::{Http, Middleware, Provider},
    utils::keccak256,
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

    /// Get ERC-20 token balance using balanceOf(address) call
    pub async fn get_token_balance(
        &self,
        token_address: &str,
        account_address: &str,
        block_number: Option<u64>,
    ) -> Result<String> {
        let token_contract = token_address
            .parse::<H160>()
            .context(format!("Invalid token contract address: {}", token_address))?;

        let account = account_address
            .parse::<H160>()
            .context(format!("Invalid account address: {}", account_address))?;

        // Encode balanceOf(address) function call
        // Function selector: first 4 bytes of keccak256("balanceOf(address)")
        let function_selector = &keccak256("balanceOf(address)".as_bytes())[0..4];
        
        // Encode the address parameter (32 bytes, left-padded)
        let mut data = Vec::with_capacity(36);
        data.extend_from_slice(function_selector);
        data.extend_from_slice(&[0u8; 12]); // 12 bytes of padding
        data.extend_from_slice(account.as_bytes()); // 20 bytes address
        
        let block_id = match block_number {
            Some(num) => Some(ethers::core::types::BlockId::Number(BlockNumber::Number(
                U64::from(num),
            ))),
            None => None,
        };

        let result = self
            .provider
            .call(
                &TransactionRequest::new()
                    .to(token_contract)
                    .data(Bytes::from(data))
                    .into(),
                block_id,
            )
            .await
            .context(format!(
                "Failed to call balanceOf for token {} and account {}",
                token_address, account_address
            ))?;

        // Convert bytes result to U256 string
        if result.0.len() >= 32 {
            let balance = ethers::core::types::U256::from_big_endian(&result.0[result.0.len() - 32..]);
            Ok(balance.to_string())
        } else {
            Ok("0".to_string())
        }
    }

    /// Get ERC-20 token name using name() call
    pub async fn get_token_name(&self, token_address: &str) -> Result<Option<String>> {
        let token_contract = token_address
            .parse::<H160>()
            .context(format!("Invalid token contract address: {}", token_address))?;

        // Encode name() function call
        let function_selector = &keccak256("name()".as_bytes())[0..4];
        
        match self.provider.call(
            &TransactionRequest::new()
                .to(token_contract)
                .data(Bytes::from(function_selector.to_vec()))
                .into(),
            None,
        ).await {
            Ok(result) => {
                if result.0.len() >= 64 {
                    // Decode string from ABI encoding
                    if let Ok(decoded) = self.decode_string_return(&result.0) {
                        return Ok(Some(decoded));
                    }
                }
                Ok(None)
            }
            Err(_) => Ok(None),
        }
    }

    /// Get ERC-20 token symbol using symbol() call
    pub async fn get_token_symbol(&self, token_address: &str) -> Result<Option<String>> {
        let token_contract = token_address
            .parse::<H160>()
            .context(format!("Invalid token contract address: {}", token_address))?;

        // Encode symbol() function call
        let function_selector = &keccak256("symbol()".as_bytes())[0..4];
        
        match self.provider.call(
            &TransactionRequest::new()
                .to(token_contract)
                .data(Bytes::from(function_selector.to_vec()))
                .into(),
            None,
        ).await {
            Ok(result) => {
                if result.0.len() >= 64 {
                    // Decode string from ABI encoding
                    if let Ok(decoded) = self.decode_string_return(&result.0) {
                        return Ok(Some(decoded));
                    }
                }
                Ok(None)
            }
            Err(_) => Ok(None),
        }
    }

    /// Get ERC-20 token decimals using decimals() call
    pub async fn get_token_decimals(&self, token_address: &str) -> Result<Option<u8>> {
        let token_contract = token_address
            .parse::<H160>()
            .context(format!("Invalid token contract address: {}", token_address))?;

        // Encode decimals() function call
        let function_selector = &keccak256("decimals()".as_bytes())[0..4];
        
        match self.provider.call(
            &TransactionRequest::new()
                .to(token_contract)
                .data(Bytes::from(function_selector.to_vec()))
                .into(),
            None,
        ).await {
            Ok(result) => {
                if result.0.len() >= 32 {
                    let decimals = ethers::core::types::U256::from_big_endian(&result.0[result.0.len() - 32..]);
                    if decimals <= ethers::core::types::U256::from(255u64) {
                        return Ok(Some(decimals.as_u32() as u8));
                    }
                }
                Ok(None)
            }
            Err(_) => Ok(None),
        }
    }

    /// Helper function to decode string return value from ABI encoding
    fn decode_string_return(&self, data: &[u8]) -> Result<String> {
        if data.len() < 64 {
            return Err(anyhow::anyhow!("Invalid string data length"));
        }

        // Skip first 32 bytes (offset) and get length from next 32 bytes
        let length = ethers::core::types::U256::from_big_endian(&data[32..64]).as_usize();
        
        if data.len() < 64 + length {
            return Err(anyhow::anyhow!("Invalid string data"));
        }

        let string_bytes = &data[64..64 + length];
        String::from_utf8(string_bytes.to_vec())
            .context("Failed to decode UTF-8 string")
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
