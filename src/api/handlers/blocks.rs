use axum::{
    extract::{Path, Query},
    Extension, Json,
};
use serde_json::json;
use std::sync::Arc;

use crate::{
    database::{BlockResponse, PaginationParams},
    App,
};

/// Get recent blocks with pagination
pub async fn get_blocks(
    Query(params): Query<PaginationParams>,
    Extension(app): Extension<Arc<App>>,
) -> Json<serde_json::Value> {
    let db = &app.db;
    let limit = params.limit();
    let offset = params.offset();

    let blocks = db
        .get_recent_blocks(limit, offset)
        .await
        .unwrap_or_default();

    // Convert to BlockResponse with calculated fields
    let mut block_responses = Vec::new();
    for block in blocks {
        let mut block_response = BlockResponse::from(&block);

        // Get transactions for this block to calculate block reward
        if let Ok(transactions) = db.get_transactions_by_block(block.number).await {
            block_response.calculate_block_reward_with_transactions(&transactions);
        }

        block_responses.push(block_response);
    }

    let total = db.get_block_count().await.unwrap_or(0);
    let current_page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(10);
    let total_pages = (total as f64 / per_page as f64).ceil() as u64;
    let has_next = current_page < total_pages;

    Json(json!({
        "blocks": block_responses,
        "total": total,
        "page": current_page,
        "per_page": per_page,
        "pages": total_pages,
        "has_next": has_next
    }))
}

/// Get block by number
pub async fn get_block_by_number(
    Path(number): Path<i64>,
    Extension(app): Extension<Arc<App>>,
) -> Json<serde_json::Value> {
    let db = &app.db;

    // Try to get block from DB
    if let Ok(Some(block)) = db.get_block_by_number(number).await {
        // Convert to BlockResponse with calculated fields
        let mut block_response = BlockResponse::from(&block);

        // Get transactions for this block to calculate block reward
        if let Ok(transactions) = db.get_transactions_by_block(number).await {
            block_response.calculate_block_reward_with_transactions(&transactions);

            return Json(json!({
                "block": block_response,
                "transactions": transactions
            }));
        }

        return Json(json!({
            "block": block_response,
            "transactions": []
        }));
    }

    // Block not found in our DB, try getting from RPC
    if let Ok(Some(eth_block)) = app.rpc.get_block_by_number(number as u64).await {
        return Json(json!({
            "block": {
                "number": eth_block.number.map(|n| n.as_u64()).unwrap_or_default(),
                "hash": eth_block.hash.map(|h| format!("{:?}", h)).unwrap_or_default(),
                "parent_hash": format!("{:?}", eth_block.parent_hash),
                "timestamp": eth_block.timestamp.as_u64(),
                "gas_used": eth_block.gas_used.as_u64(),
                "gas_limit": eth_block.gas_limit.as_u64(),
                "transaction_count": eth_block.transactions.len(),
            },
            "transactions": [],
            "note": "Block not yet indexed, basic info retrieved from blockchain"
        }));
    }

    // Neither in DB nor on chain
    Json(json!({
        "error": "Block not found"
    }))
}

/// Get recent blocks since a specific block number (delta updates)
pub async fn get_blocks_since(
    Query(params): Query<std::collections::HashMap<String, String>>,
    Extension(app): Extension<Arc<App>>,
) -> Json<serde_json::Value> {
    let db = &app.db;

    let since_block = params
        .get("since")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    // Get blocks with number > since_block, ordered by block_number DESC, limit 5
    let blocks = match sqlx::query_as::<_, crate::database::Block>(
        r#"
        SELECT number, hash, parent_hash, timestamp, gas_used, gas_limit, transaction_count,
               miner, total_difficulty, size_bytes, base_fee_per_gas, extra_data, state_root,
               nonce, withdrawals_root, blob_gas_used, excess_blob_gas, withdrawal_count,
               slot, proposer_index, epoch, slot_root, parent_root, beacon_deposit_count,
               graffiti, randao_reveal, randao_mix
        FROM blocks 
        WHERE number > ? 
        ORDER BY number DESC 
        LIMIT 5
        "#,
    )
    .bind(since_block)
    .fetch_all(&db.pool)
    .await
    {
        Ok(blocks) => blocks,
        Err(_) => vec![],
    };

    Json(json!({
        "blocks": blocks,
        "since": since_block,
        "timestamp": chrono::Utc::now().timestamp()
    }))
}

/// Get recent transactions since a specific transaction hash (delta updates)
pub async fn get_transactions_since(
    Query(params): Query<std::collections::HashMap<String, String>>,
    Extension(app): Extension<Arc<App>>,
) -> Json<serde_json::Value> {
    let db = &app.db;

    let since_hash = params.get("since").cloned().unwrap_or_default();

    let transactions = if since_hash.is_empty() {
        // First load - get latest 5 transactions
        db.get_recent_transactions(5, 0).await.unwrap_or_default()
    } else {
        // Get transactions newer than the provided hash
        match sqlx::query_as::<_, crate::database::Transaction>(
            r#"
            SELECT hash, block_number, from_address, to_address, value, gas_used, gas_price, status, transaction_index
            FROM transactions
            WHERE hash = ?
            "#,
        )
        .bind(&since_hash)
        .fetch_optional(&db.pool)
        .await {
            Ok(Some(ref_tx)) => {
                // Found reference transaction, get newer ones
                match sqlx::query_as::<_, crate::database::Transaction>(
                    r#"
                    SELECT hash, block_number, from_address, to_address, value, gas_used, gas_price, status, transaction_index
                    FROM transactions
                    WHERE (block_number > ?)
                       OR (block_number = ? AND transaction_index > ?)
                    ORDER BY block_number DESC, transaction_index DESC
                    LIMIT 5
                    "#,
                )
                .bind(ref_tx.block_number)
                .bind(ref_tx.block_number)
                .bind(ref_tx.transaction_index)
                .fetch_all(&db.pool)
                .await {
                    Ok(txs) => txs,
                    Err(_) => vec![]
                }
            }
            _ => {
                // Hash not found, return empty result
                vec![]
            }
        }
    };

    Json(json!({
        "transactions": transactions,
        "since": since_hash,
        "timestamp": chrono::Utc::now().timestamp()
    }))
}
