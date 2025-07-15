use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

use crate::{
    config::AppConfig,
    executor::{BeaconRpcOperation, BeaconRpcResponse, RpcExecutor},
};

/// Beacon Chain client for fetching consensus layer data
pub struct BeaconClient {
    client: Client,
    base_url: String,
    executor: RpcExecutor<BeaconRpcOperation, BeaconRpcResponse>,
}

/// Beacon block header response from Beacon API
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BeaconBlockHeader {
    pub slot: String,
    pub proposer_index: String,
    pub parent_root: String,
    pub state_root: String,
    pub body_root: String,
}

/// Beacon block response from Beacon API
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BeaconBlock {
    pub slot: String,
    pub proposer_index: String,
    pub parent_root: String,
    pub state_root: String,
    pub body: BeaconBlockBody,
}

/// Beacon block body
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BeaconBlockBody {
    pub randao_reveal: String,
    pub graffiti: String,
    pub proposer_slashings: Vec<serde_json::Value>,
    pub attester_slashings: Vec<serde_json::Value>,
    pub attestations: Vec<serde_json::Value>,
    pub deposits: Vec<serde_json::Value>,
    pub voluntary_exits: Vec<serde_json::Value>,
    pub execution_payload: Option<ExecutionPayload>,
}

/// Execution payload (links consensus and execution layers)
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ExecutionPayload {
    pub parent_hash: String,
    pub fee_recipient: String,
    pub state_root: String,
    pub receipts_root: String,
    pub logs_bloom: String,
    pub prev_randao: String,
    pub block_number: String,
    pub gas_limit: String,
    pub gas_used: String,
    pub timestamp: String,
    pub extra_data: String,
    pub base_fee_per_gas: String,
    pub block_hash: String,
    pub transactions: Vec<String>,
    pub withdrawals: Option<Vec<serde_json::Value>>,
    pub blob_gas_used: Option<String>,
    pub excess_blob_gas: Option<String>,
}

/// API response wrapper for beacon blocks (v2 endpoint)
#[derive(Debug, Deserialize)]
struct ApiResponse<T> {
    data: ApiResponseData<T>,
}

/// API response data wrapper
#[derive(Debug, Deserialize)]
struct ApiResponseData<T> {
    message: T,
}

/// API response wrapper for beacon headers (v1 endpoint)
#[derive(Debug, Deserialize)]
struct ApiHeaderResponse<T> {
    data: T,
}

impl BeaconClient {
    /// Create new Beacon client with rate limiting
    pub fn new(beacon_url: &str, config: &AppConfig) -> Self {
        let client = Client::new();
        let base_url = beacon_url.trim_end_matches('/').to_string();

        // Clone for the closure
        let client_clone = client.clone();
        let base_url_clone = base_url.clone();

        let executor = RpcExecutor::new(
            "Beacon".to_string(),
            config.beacon_rpc_max_concurrent,
            config.beacon_rpc_min_interval_ms,
            move |operation| {
                let client = client_clone.clone();
                let base_url = base_url_clone.clone();
                async move { Self::execute_beacon_operation(client, base_url, operation).await }
            },
        );

        Self {
            client,
            base_url,
            executor,
        }
    }

    /// Execute a beacon operation (internal implementation)
    async fn execute_beacon_operation(
        client: Client,
        base_url: String,
        operation: BeaconRpcOperation,
    ) -> Result<BeaconRpcResponse> {
        match operation {
            BeaconRpcOperation::GetBeaconDataForBlock(block_number) => {
                debug!("Fetching beacon data for block {}", block_number);

                // First, get the slot for this execution block
                let slot = match Self::get_slot_for_execution_block(block_number).await {
                    Ok(Some(slot)) => slot,
                    Ok(None) => {
                        debug!("No slot found for execution block {}", block_number);
                        return Ok(BeaconRpcResponse::BeaconDataForBlock(serde_json::json!({
                            "slot": null,
                            "proposer_index": null,
                            "epoch": null,
                            "slot_root": null,
                            "parent_root": null,
                            "beacon_deposit_count": null,
                            "graffiti": null,
                            "randao_reveal": null,
                            "randao_mix": null
                        })));
                    }
                    Err(e) => {
                        debug!("Error getting slot for block {}: {}", block_number, e);
                        return Ok(BeaconRpcResponse::BeaconDataForBlock(serde_json::json!({
                            "slot": null,
                            "proposer_index": null,
                            "epoch": null,
                            "slot_root": null,
                            "parent_root": null,
                            "beacon_deposit_count": null,
                            "graffiti": null,
                            "randao_reveal": null,
                            "randao_mix": null
                        })));
                    }
                };

                // Get beacon block data for this slot
                let beacon_data =
                    match Self::get_beacon_block_for_slot(client.clone(), base_url.clone(), slot)
                        .await
                    {
                        Ok(Some(block_data)) => {
                            let epoch = slot / 32; // 32 slots per epoch

                            serde_json::json!({
                                "slot": slot,
                                "proposer_index": block_data.get("proposer_index"),
                                "epoch": epoch,
                                "slot_root": block_data.get("state_root"),
                                "parent_root": block_data.get("parent_root"),
                                "beacon_deposit_count": block_data
                                    .get("body")
                                    .and_then(|body| body.get("deposits"))
                                    .and_then(|deposits| deposits.as_array())
                                    .map(|arr| arr.len() as i64),
                                "graffiti": block_data
                                    .get("body")
                                    .and_then(|body| body.get("graffiti")),
                                "randao_reveal": block_data
                                    .get("body")
                                    .and_then(|body| body.get("randao_reveal")),
                                "randao_mix": null
                            })
                        }
                        Ok(None) => {
                            debug!("No beacon block found for slot {}", slot);
                            serde_json::json!({
                                "slot": slot,
                                "proposer_index": null,
                                "epoch": slot / 32,
                                "slot_root": null,
                                "parent_root": null,
                                "beacon_deposit_count": null,
                                "graffiti": null,
                                "randao_reveal": null,
                                "randao_mix": null
                            })
                        }
                        Err(e) => {
                            debug!("Error getting beacon block for slot {}: {}", slot, e);
                            serde_json::json!({
                                "slot": slot,
                                "proposer_index": null,
                                "epoch": slot / 32,
                                "slot_root": null,
                                "parent_root": null,
                                "beacon_deposit_count": null,
                                "graffiti": null,
                                "randao_reveal": null,
                                "randao_mix": null
                            })
                        }
                    };

                Ok(BeaconRpcResponse::BeaconDataForBlock(beacon_data))
            }
            BeaconRpcOperation::TestConnection => {
                let url = format!("{}/eth/v1/node/health", base_url);
                match client.get(&url).send().await {
                    Ok(response) => {
                        if response.status().is_success() {
                            info!("Successfully connected to Beacon node");
                            Ok(BeaconRpcResponse::TestConnection(()))
                        } else {
                            error!("Beacon connection failed: {}", response.status());
                            Err(anyhow::anyhow!("Beacon connection failed"))
                        }
                    }
                    Err(e) => {
                        error!("Beacon connection error: {}", e);
                        Err(anyhow::anyhow!("Beacon connection error: {}", e))
                    }
                }
            }
            _ => {
                // For now, other operations return default values
                Ok(BeaconRpcResponse::BeaconDataForBlock(serde_json::json!({})))
            }
        }
    }

    /// Test connection to Beacon node
    pub async fn test_connection(&self) -> Result<()> {
        match self
            .executor
            .execute(BeaconRpcOperation::TestConnection)
            .await?
        {
            BeaconRpcResponse::TestConnection(_) => Ok(()),
            _ => Err(anyhow::anyhow!("Unexpected response type")),
        }
    }

    /// Get beacon data for a specific execution block
    pub async fn get_beacon_data_for_block(&self, block_number: u64) -> Result<serde_json::Value> {
        match self
            .executor
            .execute(BeaconRpcOperation::GetBeaconDataForBlock(block_number))
            .await?
        {
            BeaconRpcResponse::BeaconDataForBlock(data) => Ok(data),
            _ => Err(anyhow::anyhow!("Unexpected response type")),
        }
    }

    /// Get beacon block header by slot
    pub async fn get_block_header(&self, slot: u64) -> Result<Option<BeaconBlockHeader>> {
        let url = format!("{}/eth/v1/beacon/headers/{}", self.base_url, slot);
        info!("Fetching beacon block header from URL: {}", url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context(format!("Failed to make request to {}", url))?;

        debug!("Beacon header response status: {}", response.status());

        if response.status() == 404 {
            warn!("Beacon header not found for slot {}", slot);
            return Ok(None);
        }

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read error".to_string());
            error!(
                "Beacon header request failed with status {}: {}",
                status, error_text
            );
            return Err(anyhow::anyhow!("HTTP {} error: {}", status, error_text));
        }

        let response_text = response
            .text()
            .await
            .context("Failed to read beacon header response body")?;
        debug!("Beacon header response body: {}", response_text);

        let api_response: ApiHeaderResponse<BeaconBlockHeader> =
            match serde_json::from_str(&response_text) {
                Ok(response) => {
                    debug!("Successfully parsed beacon header JSON");
                    response
                }
                Err(e) => {
                    warn!("Failed to parse beacon header JSON: {}", e);
                    return Err(anyhow::anyhow!(
                        "Failed to parse beacon header response: {}",
                        e
                    ));
                }
            };
        Ok(Some(api_response.data))
    }

    /// Get beacon block by slot  
    pub async fn get_block(&self, slot: u64) -> Result<Option<BeaconBlock>> {
        let url = format!("{}/eth/v2/beacon/blocks/{}", self.base_url, slot);
        info!("Fetching beacon block from URL: {}", url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context(format!("Failed to make request to {}", url))?;

        debug!("Beacon block response status: {}", response.status());

        if response.status() == 404 {
            warn!("Beacon block not found for slot {}", slot);
            return Ok(None);
        }

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read error".to_string());
            error!(
                "Beacon block request failed with status {}: {}",
                status, error_text
            );
            return Err(anyhow::anyhow!("HTTP {} error: {}", status, error_text));
        }

        let response_text = response
            .text()
            .await
            .context("Failed to read beacon block response body")?;
        info!(
            "Beacon block response body (first 500 chars): {}",
            if response_text.len() > 500 {
                &response_text[..500]
            } else {
                &response_text
            }
        );

        let api_response: ApiResponse<BeaconBlock> = match serde_json::from_str(&response_text) {
            Ok(response) => {
                debug!("Successfully parsed beacon block JSON");
                response
            }
            Err(e) => {
                warn!("Failed to parse beacon block JSON: {}", e);
                return Err(anyhow::anyhow!(
                    "Failed to parse beacon block response: {}",
                    e
                ));
            }
        };

        info!(
            "Found beacon block with slot: {}",
            api_response.data.message.slot
        );
        Ok(Some(api_response.data.message))
    }

    /// Get slot for execution block number
    /// This requires mapping between execution and consensus layers
    pub async fn get_slot_by_execution_block(&self, block_number: u64) -> Result<Option<u64>> {
        // For post-merge blocks, we can estimate slot based on block number
        // The merge happened at block 15537394 and slot 4700013
        const MERGE_BLOCK: u64 = 15537394;
        const MERGE_SLOT: u64 = 4700013;

        if block_number < MERGE_BLOCK {
            return Ok(None); // Pre-merge blocks don't have slots
        }

        // Estimate slot based on block progression
        // This is approximate and should be refined with actual beacon state
        let estimated_slot = MERGE_SLOT + (block_number - MERGE_BLOCK);
        Ok(Some(estimated_slot))
    }

    /// Calculate epoch from slot
    pub fn slot_to_epoch(slot: u64) -> u64 {
        slot / 32 // 32 slots per epoch
    }

    /// Get beacon chain deposit count
    pub async fn get_deposit_count(&self) -> Result<u64> {
        let url = format!("{}/eth/v1/beacon/deposit_snapshot", self.base_url);

        let response = self.client.get(&url).send().await?;
        let data: serde_json::Value = response.json().await?;

        if let Some(count) = data["data"]["deposit_count"].as_str() {
            Ok(count.parse()?)
        } else {
            Ok(0)
        }
    }

    /// Get slot for execution block using slot estimation
    async fn get_slot_for_execution_block(block_number: u64) -> Result<Option<u64>> {
        // For post-merge blocks, estimate slot based on block number
        const MERGE_BLOCK: u64 = 15537394;
        const MERGE_SLOT: u64 = 4700013;

        if block_number < MERGE_BLOCK {
            return Ok(None); // Pre-merge blocks don't have slots
        }

        // Estimate slot based on block progression
        let estimated_slot = MERGE_SLOT + (block_number - MERGE_BLOCK);
        Ok(Some(estimated_slot))
    }

    /// Get beacon block data for a specific slot
    async fn get_beacon_block_for_slot(
        client: Client,
        base_url: String,
        slot: u64,
    ) -> Result<Option<serde_json::Value>> {
        let url = format!("{}/eth/v2/beacon/blocks/{}", base_url, slot);

        let response = match client.get(&url).send().await {
            Ok(response) => response,
            Err(e) => {
                debug!("Failed to fetch beacon block for slot {}: {}", slot, e);
                return Ok(None);
            }
        };

        if response.status() == 404 {
            debug!("Beacon block not found for slot {}", slot);
            return Ok(None);
        }

        if !response.status().is_success() {
            debug!(
                "Beacon block request failed with status: {}",
                response.status()
            );
            return Ok(None);
        }

        let response_json: serde_json::Value = match response.json().await {
            Ok(json) => json,
            Err(e) => {
                debug!("Failed to parse beacon block response: {}", e);
                return Ok(None);
            }
        };

        // Extract the block data from the response
        if let Some(data) = response_json.get("data") {
            if let Some(message) = data.get("message") {
                return Ok(Some(message.clone()));
            }
        }

        Ok(None)
    }
}

/// Beacon chain data that can be associated with an execution block
#[derive(Debug, Clone)]
pub struct BeaconBlockData {
    pub slot: Option<i64>,
    pub proposer_index: Option<i64>,
    pub epoch: Option<i64>,
    pub slot_root: Option<String>,
    pub parent_root: Option<String>,
    pub beacon_deposit_count: Option<i64>,
    pub graffiti: Option<String>,
    pub randao_reveal: Option<String>,
    pub randao_mix: Option<String>,
}
