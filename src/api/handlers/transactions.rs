use axum::{
    extract::{Path, Query},
    Extension, Json,
};
use serde_json::json;
use std::sync::Arc;

use crate::{
    database::{PaginationParams, Transaction},
    App,
};

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

    Json(json!({
        "transactions": txs,
        "total": total,
        "page": params.page.unwrap_or(1),
        "per_page": params.per_page.unwrap_or(10),
        "pages": (total as f64 / params.limit() as f64).ceil() as u64
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

    // Get only the 5 most recent transactions, ordered by block and transaction index
    let txs = db.get_recent_transactions(5, 0).await.unwrap_or_default();

    Json(json!({
        "transactions": txs,
        "timestamp": chrono::Utc::now().timestamp(),
        "count": txs.len()
    }))
}
