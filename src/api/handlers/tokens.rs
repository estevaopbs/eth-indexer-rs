use crate::App;
use axum::{extract::Query, response::Json, Extension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::error;

#[derive(Debug, Deserialize)]
pub struct TokenBalanceQuery {
    pub account: String,
    pub token: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TokenBalanceResponse {
    pub token_address: String,
    pub token_name: Option<String>,
    pub token_symbol: Option<String>,
    pub token_decimals: Option<u8>,
    pub balance: String,
    pub last_updated_block: i64,
}

/// Get token balances for an account
pub async fn get_token_balances(
    Query(params): Query<TokenBalanceQuery>,
    Extension(app): Extension<Arc<App>>,
) -> Json<Value> {
    let account_address = params.account;

    // If specific token requested
    if let Some(token_address) = params.token {
        match app.db.get_token_balance(&account_address, &token_address).await {
            Ok(Some(balance)) => {
                // Get token info
                match app.db.get_token_by_address(&token_address).await {
                    Ok(Some(token)) => {
                        let response = TokenBalanceResponse {
                            token_address: token.address,
                            token_name: token.name,
                            token_symbol: token.symbol,
                            token_decimals: token.decimals,
                            balance: balance.balance,
                            last_updated_block: balance.last_updated_block,
                        };
                        return Json(json!({ "balance": response }));
                    }
                    Ok(None) => {
                        return Json(json!({ "error": "Token not found" }));
                    }
                    Err(e) => {
                        error!("Failed to get token info: {}", e);
                        return Json(json!({ "error": "Failed to get token info" }));
                    }
                }
            }
            Ok(None) => {
                return Json(json!({ "error": "Token balance not found" }));
            }
            Err(e) => {
                error!("Failed to get token balance: {}", e);
                return Json(json!({ "error": "Failed to get token balance" }));
            }
        }
    }

    // Get all token balances for the account
    match app.token_service.get_account_token_info(&account_address).await {
        Ok(token_balances) => {
            let balances: Vec<TokenBalanceResponse> = token_balances
                .into_iter()
                .map(|(token, balance)| TokenBalanceResponse {
                    token_address: token.address,
                    token_name: token.name,
                    token_symbol: token.symbol,
                    token_decimals: token.decimals,
                    balance: balance.balance,
                    last_updated_block: balance.last_updated_block,
                })
                .collect();

            Json(json!({
                "account": account_address,
                "balances": balances,
                "total_tokens": balances.len()
            }))
        }
        Err(e) => {
            error!("Failed to get account token balances: {}", e);
            Json(json!({ "error": "Failed to get token balances" }))
        }
    }
}

/// Get token holders for a specific token
pub async fn get_token_holders(
    Query(params): Query<serde_json::Value>,
    Extension(app): Extension<Arc<App>>,
) -> Json<Value> {
    let token_address = match params.get("token").and_then(|v| v.as_str()) {
        Some(addr) => addr,
        None => return Json(json!({ "error": "Token address is required" })),
    };

    let offset = params
        .get("offset")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let limit = params
        .get("limit")
        .and_then(|v| v.as_i64())
        .unwrap_or(50)
        .min(100); // Cap at 100

    match app.db.get_token_holders(token_address, offset, limit).await {
        Ok(holders) => {
            // Get token info
            match app.db.get_token_by_address(token_address).await {
                Ok(Some(token)) => {
                    Json(json!({
                        "token": {
                            "address": token.address,
                            "name": token.name,
                            "symbol": token.symbol,
                            "decimals": token.decimals
                        },
                        "holders": holders,
                        "total_holders": holders.len()
                    }))
                }
                Ok(None) => Json(json!({ "error": "Token not found" })),
                Err(e) => {
                    error!("Failed to get token info: {}", e);
                    Json(json!({ "error": "Failed to get token info" }))
                }
            }
        }
        Err(e) => {
            error!("Failed to get token holders: {}", e);
            Json(json!({ "error": "Failed to get token holders" }))
        }
    }
}

/// Get list of known tokens
pub async fn get_tokens(
    Query(params): Query<serde_json::Value>,
    Extension(app): Extension<Arc<App>>,
) -> Json<Value> {
    let offset = params
        .get("offset")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let limit = params
        .get("limit")
        .and_then(|v| v.as_i64())
        .unwrap_or(50)
        .min(100); // Cap at 100

    match app.db.get_tokens(offset, limit).await {
        Ok(tokens) => {
            Json(json!({
                "tokens": tokens,
                "total": tokens.len()
            }))
        }
        Err(e) => {
            error!("Failed to get tokens: {}", e);
            Json(json!({ "error": "Failed to get tokens" }))
        }
    }
}
