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
    
    // Get total network transactions
    let base_network_transactions = network_stats.get_total_network_transactions().await.unwrap_or(0);
    
    // Adjust total_network_transactions based on start_block configuration
    let total_network_transactions = if app.config.start_block.unwrap_or(0) < 0 {
        // When START_BLOCK=-1, total network should match blockchain total
        // Get current blockchain total from database
        let db_transactions = app.db.get_transaction_count().await.unwrap_or(0);
        let historical_count = if app.config.start_block.unwrap_or(0) > 0 {
            app.historical.get_historical_count().unwrap_or(0)
        } else {
            0
        };
        (historical_count + db_transactions) as u64
    } else {
        base_network_transactions
    };
    
    // Get total network accounts
    let total_network_accounts = network_stats.get_total_network_accounts().await.unwrap_or(0);
    
    Json(json!({
        "latest_network_block": latest_network_block,
        "total_network_transactions": total_network_transactions,
        "total_network_accounts": total_network_accounts,
        "timestamp": chrono::Utc::now().timestamp()
    }))
}
