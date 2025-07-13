use eth_indexer_rs::{api, App};
use std::sync::Arc;
use tracing::error;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,eth_indexer_rs=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Initialize the application
    let app = match App::init().await {
        Ok(app) => Arc::new(app),
        Err(e) => {
            error!("Failed to initialize application: {}", e);
            return Err(e);
        }
    };

    // Start the indexer and API server in parallel
    let app_clone = app.clone();
    let indexer_handle = tokio::spawn(async move {
        if let Err(e) = app_clone.start().await {
            error!("Failed to start indexer: {}", e);
        }
    });

    let api_handle = tokio::spawn(async move {
        if let Err(e) = api::start_server(app).await {
            error!("Failed to start API server: {}", e);
        }
    });

    // Wait for both to complete (they should run indefinitely)
    let _ = tokio::try_join!(indexer_handle, api_handle);

    Ok(())
}
