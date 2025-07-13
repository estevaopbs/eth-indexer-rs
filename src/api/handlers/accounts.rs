use axum::{extract::Path, Extension, Json};
use serde_json::json;
use std::sync::Arc;

use crate::{database::Account, App};

/// Get account by address
pub async fn get_account(
    Path(address): Path<String>,
    Extension(app): Extension<Arc<App>>,
) -> Json<serde_json::Value> {
    let db = &app.db;

    // Get account from DB
    if let Ok(Some(account)) = db.get_account_by_address(&address).await {
        return Json(json!({
            "account": account
        }));
    }

    // Account not found in our DB, try getting from RPC
    match app.rpc.get_balance(&address, None).await {
        Ok(balance) => {
            let account = Account {
                address: address.clone(),
                balance,
                transaction_count: 0,
                first_seen_block: 0,
                last_seen_block: 0,
            };

            return Json(json!({
                "account": account,
                "note": "Account not yet indexed, basic info retrieved from blockchain"
            }));
        }
        Err(_) => {
            return Json(json!({
                "error": "Account not found or invalid address"
            }));
        }
    }
}
