use axum::{extract::{Path, Query}, Extension, Json};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::{database::Account, App};

#[derive(Deserialize)]
pub struct AccountsQuery {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub sort: Option<String>,
    pub order: Option<String>,
}

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

/// Get accounts with pagination and sorting
pub async fn get_accounts(
    Query(query): Query<AccountsQuery>,
    Extension(app): Extension<Arc<App>>,
) -> Json<serde_json::Value> {
    let db = &app.db;
    
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(50).min(100);
    let sort = query.sort.unwrap_or_else(|| "balance".to_string());
    let order = query.order.unwrap_or_else(|| "desc".to_string());
    
    let offset = (page - 1) * per_page;
    
    // Build the SQL query based on sort and order
    let order_clause = match sort.as_str() {
        "balance" => "balance",
        "transaction_count" => "transaction_count", 
        "first_seen" => "first_seen_block",
        "last_activity" => "last_seen_block",
        _ => "balance", // default
    };
    
    let order_direction = match order.as_str() {
        "asc" => "ASC",
        _ => "DESC", // default desc
    };
    
    let query_str = format!(
        "SELECT address, balance, transaction_count, first_seen_block, last_seen_block 
         FROM accounts 
         ORDER BY {} {} 
         LIMIT {} OFFSET {}",
        order_clause, order_direction, per_page + 1, offset
    );
    
    match sqlx::query_as::<_, Account>(&query_str)
        .fetch_all(&db.pool)
        .await
    {
        Ok(mut accounts) => {
            let has_next = accounts.len() > per_page as usize;
            if has_next {
                accounts.pop(); // Remove the extra item
            }
            
            // Add account_type field based on some heuristics
            let accounts_with_type: Vec<serde_json::Value> = accounts
                .into_iter()
                .map(|account| {
                    let account_type = if account.transaction_count > 0 {
                        "eoa" // Externally Owned Account
                    } else {
                        "unknown"
                    };
                    
                    json!({
                        "address": account.address,
                        "balance": account.balance,
                        "transaction_count": account.transaction_count,
                        "account_type": account_type,
                        "first_seen": account.first_seen_block,
                        "last_activity": account.last_seen_block
                    })
                })
                .collect();
            
            Json(json!({
                "accounts": accounts_with_type,
                "has_next": has_next,
                "page": page,
                "per_page": per_page
            }))
        }
        Err(e) => {
            Json(json!({
                "error": format!("Failed to fetch accounts: {}", e),
                "accounts": [],
                "has_next": false,
                "page": page,
                "per_page": per_page
            }))
        }
    }
}
