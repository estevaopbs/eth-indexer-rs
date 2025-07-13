use axum::{Extension, Json};
use serde_json::{json, Value};
use std::sync::Arc;

use crate::App;

/// Health check endpoint
pub async fn health_check(Extension(app): Extension<Arc<App>>) -> Json<Value> {
    let is_rpc_connected = app.rpc.check_connection().await.unwrap_or(false);
    let is_indexer_running = app.indexer.is_running();

    Json(json!({
        "status": "ok",
        "rpc_connected": is_rpc_connected,
        "indexer_running": is_indexer_running,
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
