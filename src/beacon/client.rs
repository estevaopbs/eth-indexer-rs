use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error, info};

/// Beacon Chain client for fetching consensus layer data
pub struct BeaconClient {
    client: Client,
    base_url: String,
}

/// Beacon block header response from Beacon API
#[derive(Debug, Deserialize, Serialize)]
pub struct BeaconBlockHeader {
    pub slot: String,
    pub proposer_index: String,
    pub parent_root: String,
    pub state_root: String,
    pub body_root: String,
}

/// Beacon block response from Beacon API
#[derive(Debug, Deserialize, Serialize)]
pub struct BeaconBlock {
    pub slot: String,
    pub proposer_index: String,
    pub parent_root: String,
    pub state_root: String,
    pub body: BeaconBlockBody,
}

/// Beacon block body
#[derive(Debug, Deserialize, Serialize)]
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
#[derive(Debug, Deserialize, Serialize)]
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

/// API response wrapper
#[derive(Debug, Deserialize)]
struct ApiResponse<T> {
    data: T,
}

impl BeaconClient {
    /// Create new Beacon client
    pub fn new(beacon_url: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: beacon_url.trim_end_matches('/').to_string(),
        }
    }

    /// Test connection to Beacon node
    pub async fn test_connection(&self) -> Result<()> {
        let url = format!("{}/eth/v1/node/health", self.base_url);
        let response = self.client.get(&url).send().await?;
        
        if response.status().is_success() {
            info!("Successfully connected to Beacon node at {}", self.base_url);
            Ok(())
        } else {
            error!("Failed to connect to Beacon node: {}", response.status());
            Err(anyhow::anyhow!("Beacon node connection failed"))
        }
    }

    /// Get beacon block header by slot
    pub async fn get_block_header(&self, slot: u64) -> Result<Option<BeaconBlockHeader>> {
        let url = format!("{}/eth/v1/beacon/headers/{}", self.base_url, slot);
        debug!("Fetching beacon block header for slot {}", slot);

        let response = self.client.get(&url).send().await?;
        
        if response.status() == 404 {
            return Ok(None);
        }

        let api_response: ApiResponse<BeaconBlockHeader> = response
            .json()
            .await
            .context("Failed to parse beacon block header response")?;

        Ok(Some(api_response.data))
    }

    /// Get beacon block by slot  
    pub async fn get_block(&self, slot: u64) -> Result<Option<BeaconBlock>> {
        let url = format!("{}/eth/v2/beacon/blocks/{}", self.base_url, slot);
        debug!("Fetching beacon block for slot {}", slot);

        let response = self.client.get(&url).send().await?;
        
        if response.status() == 404 {
            return Ok(None);
        }

        let api_response: ApiResponse<BeaconBlock> = response
            .json()
            .await
            .context("Failed to parse beacon block response")?;

        Ok(Some(api_response.data))
    }

    /// Get slot for execution block number
    /// This requires mapping between execution and consensus layers
    pub async fn get_slot_by_execution_block(&self, block_number: u64) -> Result<Option<u64>> {
        // For post-merge blocks, we can estimate slot based on block number
        // The merge happened at block 15537394 and slot 4700013
        const MERGE_BLOCK: u64 = 15537394;
        const MERGE_SLOT: u64 = 4700013;
        const SECONDS_PER_SLOT: u64 = 12;

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

impl BeaconClient {
    /// Get beacon data for execution block
    pub async fn get_beacon_data_for_block(&self, block_number: u64) -> Result<BeaconBlockData> {
        // Get estimated slot for this execution block
        let slot = match self.get_slot_by_execution_block(block_number).await? {
            Some(s) => s,
            None => {
                // Pre-merge block, return empty beacon data
                return Ok(BeaconBlockData {
                    slot: None,
                    proposer_index: None,
                    epoch: None,
                    slot_root: None,
                    parent_root: None,
                    beacon_deposit_count: None,
                    graffiti: None,
                    randao_reveal: None,
                    randao_mix: None,
                });
            }
        };

        // Try to get beacon block for this slot
        let beacon_block = self.get_block(slot).await?;
        
        if let Some(block) = beacon_block {
            let slot_num = block.slot.parse::<u64>().unwrap_or(0);
            let proposer_index = block.proposer_index.parse::<u64>().unwrap_or(0);
            let epoch = Self::slot_to_epoch(slot_num);
            
            // Get deposit count
            let deposit_count = self.get_deposit_count().await.unwrap_or(0);
            
            // Extract graffiti and randao from block body
            let graffiti = if block.body.graffiti.starts_with("0x") && block.body.graffiti.len() > 2 {
                // Decode hex graffiti to UTF-8 if possible
                hex::decode(&block.body.graffiti[2..])
                    .ok()
                    .and_then(|bytes| String::from_utf8(bytes).ok())
                    .or_else(|| Some(block.body.graffiti.clone()))
            } else {
                Some(block.body.graffiti.clone())
            };

            Ok(BeaconBlockData {
                slot: Some(slot_num as i64),
                proposer_index: Some(proposer_index as i64),
                epoch: Some(epoch as i64),
                slot_root: Some(block.state_root),
                parent_root: Some(block.parent_root),
                beacon_deposit_count: Some(deposit_count as i64),
                graffiti,
                randao_reveal: Some(block.body.randao_reveal),
                randao_mix: block.body.execution_payload
                    .as_ref()
                    .map(|payload| payload.prev_randao.clone()),
            })
        } else {
            // Slot not found, return partial data
            Ok(BeaconBlockData {
                slot: Some(slot as i64),
                proposer_index: None,
                epoch: Some(Self::slot_to_epoch(slot) as i64),
                slot_root: None,
                parent_root: None,
                beacon_deposit_count: None,
                graffiti: None,
                randao_reveal: None,
                randao_mix: None,
            })
        }
    }
}
