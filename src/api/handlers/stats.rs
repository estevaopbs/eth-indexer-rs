use crate::{database::IndexerStats, App};
use axum::{Extension, Json};
use serde_json::json;
use std::sync::Arc;

/// Get indexer statistics
pub async fn get_stats(Extension(app): Extension<Arc<App>>) -> Json<IndexerStats> {
    let db = &app.db;
    let latest_block = db
        .get_latest_block_number()
        .await
        .unwrap_or(None)
        .unwrap_or(-1);
    let total_blocks = db.get_block_count().await.unwrap_or(0);
    let total_transactions_indexed = db.get_transaction_count().await.unwrap_or(0);
    let total_transactions_declared = db.get_declared_transaction_count().await.unwrap_or(0);

    // Get historical transaction count before our start block
    let start_block = app.config.start_block.unwrap_or(0);

    let historical_count = if start_block > 0 {
        // Use the historical transaction service
        app.historical.get_historical_count().unwrap_or(0)
    } else {
        0
    };

    // Add historical count to our indexed transactions for total
    let total_transactions = historical_count + total_transactions_indexed;
    let total_transactions_declared_with_history = historical_count + total_transactions_declared;

    // Calculate total_blockchain_transactions based on start_block configuration
    let total_blockchain_transactions = if start_block < 0 {
        // When START_BLOCK=-1, we started from the latest block, so total should equal indexed transactions
        // In this case, we don't have a reliable way to get total network transactions
        // so we use what we have: historical + indexed
        historical_count + total_transactions_indexed
    } else if historical_count > 0 {
        // Normal case: historical + indexed (only when historical data is available)
        historical_count + total_transactions_indexed
    } else {
        // No historical data available - set to 0 to indicate unavailable
        0
    };

    // Count accounts (if the table exists)
    let total_accounts: i64 = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM accounts")
        .fetch_one(&db.pool)
        .await
        .map(|r| r.0)
        .unwrap_or(0);

    // Get indexer status
    let indexer_status = if app.indexer.is_running() {
        "running"
    } else {
        "stopped"
    };

    // Calculate sync percentage (assume we start from genesis block)
    let latest_chain_block = app
        .rpc
        .get_latest_block_number()
        .await
        .unwrap_or(latest_block as u64);

    let sync_percentage = if latest_chain_block > 0 {
        (latest_block as f64 / latest_chain_block as f64) * 100.0
    } else {
        0.0
    };

    // Calculate transaction indexing percentage (only for blocks we're tracking)
    // Use total_transactions_declared as the target (includes skipped transactions as expected)
    let transaction_indexing_percentage = if total_transactions_declared > 0 {
        (total_transactions_indexed as f64 / total_transactions_declared as f64) * 100.0
    } else {
        100.0
    };

    // Get current block transaction information
    let (current_block_tx_indexed, current_block_tx_declared) = db
        .get_current_block_transaction_info()
        .await
        .unwrap_or((0, 0));

    Json(IndexerStats {
        latest_block,
        total_blocks,
        total_transactions,
        total_transactions_declared: total_transactions_declared_with_history,
        total_transactions_indexed: historical_count + total_transactions_indexed,
        real_transactions_indexed: total_transactions_indexed, // Only transactions from start_block onwards
        total_blockchain_transactions,                         // Use the calculated value
        total_accounts: total_accounts as i64,
        indexer_status: indexer_status.to_string(),
        sync_percentage,
        transaction_indexing_percentage,
        start_block: start_block as i64,
        current_block_tx_indexed,
        current_block_tx_declared,
    })
}

/// Get latest network block number
pub async fn get_network_latest(Extension(app): Extension<Arc<App>>) -> Json<serde_json::Value> {
    let latest_network_block = app.rpc.get_latest_block_number().await.unwrap_or(0);

    Json(json!({
        "latest_network_block": latest_network_block
    }))
}
