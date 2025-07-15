use axum::{Extension, Json};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::App;

/// Health check endpoint
pub async fn health_check(Extension(app): Extension<Arc<App>>) -> Json<Value> {
    // Get cached health status (updated every 60 seconds in background)
    let health_status = app.health_cache.get_health_status().await;
    let is_indexer_running = app.indexer.is_running();

    Json(json!({
        "status": "ok",
        "indexer_running": is_indexer_running,
        "version": env!("CARGO_PKG_VERSION"),
        "rpc_connected": health_status.rpc_connected,
        "last_rpc_check": health_status.last_checked.elapsed().as_secs(),
    }))
}
