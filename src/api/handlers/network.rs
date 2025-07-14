use axum::{Extension, Json};
use serde_json::json;
use std::sync::Arc;
use tracing::warn;

use crate::App;

/// Get network-wide statistics
pub async fn get_network_stats(Extension(app): Extension<Arc<App>>) -> Json<serde_json::Value> {
    let network_stats = &app.network_stats;
    
    // Get latest network block
    let latest_network_block = network_stats.get_latest_network_block().await.unwrap_or(0);
    
    // Get total network accounts
    let total_network_accounts = network_stats.get_total_network_accounts().await.unwrap_or(0);
    
    Json(json!({
        "latest_network_block": latest_network_block,
        "total_network_accounts": total_network_accounts,
        "timestamp": chrono::Utc::now().timestamp()
    }))
}
