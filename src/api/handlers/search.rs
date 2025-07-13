use axum::{extract::Path, Extension, Json};
use serde_json::json;
use std::sync::Arc;

use crate::App;

/// Search for blocks, transactions, or accounts
pub async fn search(
    Path(query): Path<String>,
    Extension(app): Extension<Arc<App>>,
) -> Json<serde_json::Value> {
    let db = &app.db;
    let query = query.trim();

    // Try to parse as number for block search
    if let Ok(block_num) = query.parse::<i64>() {
        if let Ok(Some(block)) = db.get_block_by_number(block_num).await {
            return Json(json!({
                "type": "block",
                "result": block
            }));
        }
    }

    // Check if it looks like a block hash (0x followed by 64 hex chars)
    if query.starts_with("0x") && query.len() == 66 {
        // Try as block hash
        if let Ok(Some(block)) = db.get_block_by_hash(query).await {
            return Json(json!({
                "type": "block",
                "result": block
            }));
        }

        // Try as transaction hash
        if let Ok(Some(tx)) = db.get_transaction_by_hash(query).await {
            return Json(json!({
                "type": "transaction",
                "result": tx
            }));
        }
    }

    // Check if it looks like an address (0x followed by 40 hex chars)
    if query.starts_with("0x") && query.len() == 42 {
        if let Ok(Some(account)) = db.get_account_by_address(query).await {
            return Json(json!({
                "type": "account",
                "result": account
            }));
        }
    }

    // Nothing found
    Json(json!({
        "type": "unknown",
        "result": null,
        "message": "No matching block, transaction, or account found"
    }))
}
