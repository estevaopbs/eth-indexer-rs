use crate::App;
use axum::{
    routing::{get, Router},
    Extension,
};
use std::sync::Arc;
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
    trace::TraceLayer,
};
use tracing::info;

use super::handlers::*;

pub async fn create_router(app: Arc<App>) -> Router {
    let cors = CorsLayer::new()
        .allow_methods(Any)
        .allow_headers(Any)
        .allow_origin(Any);
    let api_routes = Router::new()
        .route("/health", get(health_check))
        .route("/stats", get(get_stats))
        .route("/network/latest", get(get_network_latest))
        .route("/network/stats", get(get_network_stats))
        .route("/blocks", get(get_blocks))
        .route("/blocks/since", get(get_blocks_since))
        .route("/blocks/:number", get(get_block_by_number))
        .route("/transactions", get(get_transactions))
        .route("/transactions/filtered", get(get_filtered_transactions))
        .route("/transactions/live", get(get_live_transactions))
        .route("/transactions/since", get(get_transactions_since))
        .route("/transactions/:hash", get(get_transaction_by_hash))
        .route(
            "/transactions/:hash/token-transfers",
            get(get_transaction_token_transfers),
        )
        .route("/accounts", get(get_accounts))
        .route("/accounts/filtered", get(get_filtered_accounts))
        .route("/accounts/:address", get(get_account))
        .route("/tokens", get(get_tokens))
        .route("/tokens/balances", get(get_token_balances))
        .route("/tokens/holders", get(get_token_holders))
        .route("/search/:query", get(search))
        .layer(Extension(app.clone()))
        .layer(cors.clone())
        .layer(TraceLayer::new_for_http());

    let static_files = Router::new().nest_service("/", ServeDir::new("src/web/static"));

    Router::new()
        .nest("/api", api_routes)
        .merge(static_files)
        .layer(Extension(app))
        .layer(TraceLayer::new_for_http())
}

pub async fn start_server(app: Arc<App>) -> anyhow::Result<()> {
    let addr = format!("0.0.0.0:{}", app.config.api_port);
    let router = create_router(app).await;

    info!("Starting API server on {}", addr);

    axum::Server::bind(&addr.parse()?)
        .serve(router.into_make_service())
        .await?;

    Ok(())
}
