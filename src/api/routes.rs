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

/// Create the API router
pub async fn create_router(app: Arc<App>) -> Router {
    info!("Setting up API routes");

    // CORS configuration
    let cors = CorsLayer::new()
        .allow_methods(Any)
        .allow_headers(Any)
        .allow_origin(Any);

    // API routes
    let api_routes = Router::new()
        .route("/health", get(health_check))
        .route("/stats", get(get_stats))
        .route("/network/latest", get(get_network_latest))
        .route("/blocks", get(get_blocks))
        .route("/blocks/since", get(get_blocks_since))
        .route("/blocks/:number", get(get_block_by_number))
        .route("/transactions", get(get_transactions))
        .route("/transactions/live", get(get_live_transactions))
        .route("/transactions/since", get(get_transactions_since))
        .route("/transactions/:hash", get(get_transaction_by_hash))
        .route("/accounts/:address", get(get_account))
        .route("/search/:query", get(search))
        .layer(Extension(app.clone()))
        .layer(cors.clone())
        .layer(TraceLayer::new_for_http());

    // Static file serving for frontend
    let static_files = Router::new().nest_service("/", ServeDir::new("src/web/static"));

    // Combine routes
    Router::new()
        .nest("/api", api_routes)
        .merge(static_files)
        .layer(Extension(app))
        .layer(TraceLayer::new_for_http())
}

/// Start the API server
pub async fn start_server(app: Arc<App>) -> anyhow::Result<()> {
    let addr = format!("0.0.0.0:{}", app.config.api_port);
    let router = create_router(app).await;

    info!("Starting API server on {}", addr);

    axum::Server::bind(&addr.parse()?)
        .serve(router.into_make_service())
        .await?;

    Ok(())
}
