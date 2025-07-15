use eth_indexer_rs::config::AppConfig;
use eth_indexer_rs::{api, App};
use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app_config = AppConfig::load()?;
    info!("Application configuration loaded");

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            app_config.log_level.clone(),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app = match App::init(app_config).await {
        Ok(app) => Arc::new(app),
        Err(e) => {
            error!("Failed to initialize application: {}", e);
            return Err(e);
        }
    };

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
