use axum::{
    extract::{Path, Query},
    Extension, Json,
};
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
        // Determine account type based on transaction count and blockchain state
        let account_type = determine_account_type(&account, &app).await;

        return Json(json!({
            "account": {
                "address": account.address,
                "balance": account.balance,
                "transaction_count": account.transaction_count,
                "account_type": account_type,
                "first_seen_block": account.first_seen_block,
                "last_seen_block": account.last_seen_block
            }
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

            let account_type = determine_account_type(&account, &app).await;

            return Json(json!({
                "account": {
                    "address": account.address,
                    "balance": account.balance,
                    "transaction_count": account.transaction_count,
                    "account_type": account_type,
                    "first_seen_block": account.first_seen_block,
                    "last_seen_block": account.last_seen_block
                },
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
        order_clause,
        order_direction,
        per_page + 1,
        offset
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
        Err(e) => Json(json!({
            "error": format!("Failed to fetch accounts: {}", e),
            "accounts": [],
            "has_next": false,
            "page": page,
            "per_page": per_page
        })),
    }
}

/// Get accounts with filtering
pub async fn get_filtered_accounts(
    Query(filters): Query<crate::database::AccountFilterParams>,
    Extension(app): Extension<Arc<App>>,
) -> Json<serde_json::Value> {
    let db = &app.db;

    let accounts = db.get_filtered_accounts(&filters).await.unwrap_or_default();

    let total = db.get_account_count().await.unwrap_or(0);
    let current_page = filters.page.unwrap_or(1);
    let per_page = filters.per_page.unwrap_or(10);
    let total_pages = (total as f64 / per_page as f64).ceil() as u64;
    let has_next = current_page < total_pages;

    Json(json!({
        "accounts": accounts,
        "pagination": {
            "current_page": current_page,
            "per_page": per_page,
            "total": total,
            "total_pages": total_pages,
            "has_next": has_next
        },
        "filters": {
            "account_type": filters.account_type,
            "min_balance": filters.min_balance,
            "max_balance": filters.max_balance,
            "min_tx_count": filters.min_tx_count,
            "max_tx_count": filters.max_tx_count,
            "sort": filters.sort,
            "order": filters.order
        }
    }))
}

/// Determine account type based on transaction count and blockchain state
async fn determine_account_type(account: &Account, app: &App) -> &'static str {
    // If account has made transactions, it's likely an EOA (Externally Owned Account)
    if account.transaction_count > 0 {
        return "eoa";
    }

    // Check if the address is a smart contract by getting code
    match app.rpc.get_code(&account.address, None).await {
        Ok(code) => {
            // If there's bytecode at the address, it's a contract
            if !code.is_empty() && code != "0x" {
                "contract"
            } else {
                // No code and no transactions - could be an unused EOA
                "eoa"
            }
        }
        Err(_) => {
            // RPC error - default based on transaction count
            if account.transaction_count > 0 {
                "eoa"
            } else {
                "unknown"
            }
        }
    }
}
