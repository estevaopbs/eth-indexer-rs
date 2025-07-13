#[cfg(test)]
mod tests {
    use eth_indexer_rs::{config::AppConfig, database::DatabaseService, rpc::RpcClient};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_database_connection() {
        let db_url = "sqlite::memory:";
        let result = DatabaseService::new(db_url).await;
        assert!(result.is_ok(), "Should connect to in-memory database");

        let db = result.unwrap();
        let tables = sqlx::query("SELECT name FROM sqlite_master WHERE type='table'")
            .fetch_all(&db.pool)
            .await
            .unwrap();

        // Check that tables were created
        assert!(tables.len() > 0, "Should create database tables");
    }

    #[tokio::test]
    async fn test_config_loading() {
        let config_result = AppConfig::load();
        assert!(config_result.is_ok(), "Should load default config");

        let config = config_result.unwrap();
        assert!(
            !config.database_url.is_empty(),
            "Database URL should not be empty"
        );
        assert!(
            !config.eth_rpc_url.is_empty(),
            "ETH RPC URL should not be empty"
        );
        assert!(config.api_port > 0, "API port should be > 0");
    }

    // This test is skipped by default as it requires a valid RPC endpoint
    #[tokio::test]
    #[ignore]
    async fn test_rpc_connection() {
        let config = AppConfig::load().unwrap();
        let rpc_result = RpcClient::new(&config.eth_rpc_url, config.clone());
        assert!(rpc_result.is_ok(), "Should create RPC client");

        let rpc = rpc_result.unwrap();
        let connected = rpc.check_connection().await.unwrap();
        assert!(connected, "Should connect to Ethereum node");

        let block_number = rpc.get_latest_block_number().await;
        assert!(block_number.is_ok(), "Should get latest block number");
        assert!(block_number.unwrap() > 0, "Block number should be > 0");
    }
}
