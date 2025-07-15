use crate::{database::PaginationParams, App};
use axum::{
    extract::{Path, Query},
    Extension, Json,
};
use serde_json::json;
use std::sync::Arc;

/// Get recent transactions with pagination
pub async fn get_transactions(
    Query(params): Query<PaginationParams>,
    Extension(app): Extension<Arc<App>>,
) -> Json<serde_json::Value> {
    let db = &app.db;
    let limit = params.limit();
    let offset = params.offset();

    let txs = db
        .get_recent_transactions(limit, offset)
        .await
        .unwrap_or_default();

    let total = db.get_transaction_count().await.unwrap_or(0);
    let current_page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(10);
    let total_pages = (total as f64 / per_page as f64).ceil() as u64;
    let has_next = current_page < total_pages;

    Json(json!({
        "transactions": txs,
        "pagination": {
            "current_page": current_page,
            "per_page": per_page,
            "total": total,
            "total_pages": total_pages,
            "has_next": has_next
        }
    }))
}

/// Get transactions with filtering
pub async fn get_filtered_transactions(
    Query(filters): Query<crate::database::TransactionFilterParams>,
    Extension(app): Extension<Arc<App>>,
) -> Json<serde_json::Value> {
    let db = &app.db;

    let txs = db
        .get_filtered_transactions(&filters)
        .await
        .unwrap_or_default();

    let total = db.get_transaction_count().await.unwrap_or(0);
    let current_page = filters.page.unwrap_or(1);
    let per_page = filters.per_page.unwrap_or(10);
    let total_pages = (total as f64 / per_page as f64).ceil() as u64;
    let has_next = current_page < total_pages;

    Json(json!({
        "transactions": txs,
        "pagination": {
            "current_page": current_page,
            "per_page": per_page,
            "total": total,
            "total_pages": total_pages,
            "has_next": has_next
        },
        "filters": {
            "status": filters.status,
            "min_value": filters.min_value,
            "max_value": filters.max_value,
            "from_block": filters.from_block,
            "to_block": filters.to_block
        }
    }))
}

/// Get transaction by hash
pub async fn get_transaction_by_hash(
    Path(hash): Path<String>,
    Extension(app): Extension<Arc<App>>,
) -> Json<serde_json::Value> {
    let db = &app.db;

    // Get transaction from DB
    if let Ok(Some(tx)) = db.get_transaction_by_hash(&hash).await {
        // Get logs for this transaction
        if let Ok(logs) = db.get_logs_by_transaction(&hash).await {
            return Json(json!({
                "transaction": tx,
                "logs": logs
            }));
        }
        return Json(json!({
            "transaction": tx,
            "logs": []
        }));
    }

    // Transaction not found in our DB, try getting from RPC
    if let Ok(Some(receipt)) = app.rpc.get_transaction_receipt(&hash).await {
        return Json(json!({
            "transaction": {
                "hash": format!("{:?}", receipt.transaction_hash),
                "block_number": receipt.block_number.map(|n| n.as_u64()).unwrap_or_default(),
                "status": receipt.status.map(|s| s.as_u64()).unwrap_or_default(),
                "gas_used": receipt.gas_used.map(|g| g.as_u64()).unwrap_or_default(),
            },
            "logs": [],
            "note": "Transaction not yet indexed, basic info retrieved from blockchain"
        }));
    }

    // Neither in DB nor on chain
    Json(json!({
        "error": "Transaction not found"
    }))
}

/// Get the most recent transactions (live feed)
pub async fn get_live_transactions(Extension(app): Extension<Arc<App>>) -> Json<serde_json::Value> {
    let db = &app.db;

    // Get only the 10 most recent transactions, ordered by block and transaction index
    let txs = db.get_recent_transactions(10, 0).await.unwrap_or_default();

    Json(json!({
        "transactions": txs,
        "timestamp": chrono::Utc::now().timestamp(),
        "count": txs.len()
    }))
}

/// Get token transfers for a specific transaction
pub async fn get_transaction_token_transfers(
    Path(hash): Path<String>,
    Extension(app): Extension<Arc<App>>,
) -> Json<serde_json::Value> {
    let db = &app.db;

    match db.get_token_transfers_by_transaction_hash(&hash).await {
        Ok(transfers) => {
            if transfers.is_empty() {
                Json(json!({
                    "transaction_hash": hash,
                    "token_transfers": [],
                    "count": 0
                }))
            } else {
                // Get token info for each transfer
                let mut enhanced_transfers = Vec::new();

                for transfer in transfers {
                    let token_info = db
                        .get_token_by_address(&transfer.token_address)
                        .await
                        .unwrap_or(None);

                    let enhanced_transfer = json!({
                        "id": transfer.id,
                        "transaction_hash": transfer.transaction_hash,
                        "token_address": transfer.token_address,
                        "from_address": transfer.from_address,
                        "to_address": transfer.to_address,
                        "amount": transfer.amount,
                        "block_number": transfer.block_number,
                        "token_type": transfer.token_type,
                        "token_id": transfer.token_id,
                        "token": token_info.map(|token| json!({
                            "name": token.name,
                            "symbol": token.symbol,
                            "decimals": token.decimals
                        }))
                    });

                    enhanced_transfers.push(enhanced_transfer);
                }

                Json(json!({
                    "transaction_hash": hash,
                    "token_transfers": enhanced_transfers,
                    "count": enhanced_transfers.len()
                }))
            }
        }
        Err(e) => {
            tracing::error!(
                "Failed to get token transfers for transaction {}: {}",
                hash,
                e
            );
            Json(json!({
                "error": "Failed to get token transfers",
                "transaction_hash": hash
            }))
        }
    }
}
