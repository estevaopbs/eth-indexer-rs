use axum::{Extension, Json};
use serde_json::json;
use std::sync::Arc;
use tracing::warn;

use crate::{database::IndexerStats, App};

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
        // Try to get accurate data from BigQuery if service account is configured
        if app.config.bigquery_service_account_path.is_some() {
            match app
                .rpc
                .get_historical_transaction_count_from_bigquery(start_block)
                .await
            {
                Ok(count) => count as i64,
                Err(e) => {
                    warn!("Failed to fetch historical data from BigQuery: {}", e);
                    // Fallback to our estimation
                    db.get_historical_transaction_count(start_block)
                        .await
                        .unwrap_or(0)
                }
            }
        } else {
            warn!("BigQuery service account not configured, using estimation for historical transaction count");
            // Fallback to our estimation
            db.get_historical_transaction_count(start_block)
                .await
                .unwrap_or(0)
        }
    } else {
        0
    };

    // Add historical count to our indexed transactions for total
    let total_transactions = historical_count + total_transactions_indexed;
    let total_transactions_declared_with_history = historical_count + total_transactions_declared;

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
