use eth_indexer_rs::config::AppConfig;
use eth_indexer_rs::{
    database::{Block, Transaction},
    App,
};
use tokio;

#[tokio::test]
async fn test_app_initialization_with_env() {
    std::env::set_var("DATABASE_URL", "sqlite:./data/test_indexer.db");
    let app_config = AppConfig::load().expect("Failed to load configuration from .env");
    let app_result = App::init(app_config).await;
    assert!(
        app_result.is_ok(),
        "Failed to initialize application: {:?}",
        app_result.err()
    );
    let app = app_result.unwrap();

    assert!(app.config.database_url.contains("test_indexer.db"));
    assert!(app.config.eth_rpc_url.starts_with("http"));
    assert!(app.config.beacon_rpc_url.starts_with("http"));
    assert!(app.config.api_port > 0);
}

#[tokio::test]
async fn test_database_operations() {
    std::env::set_var("DATABASE_URL", "./data/test_db_ops.db");
    let app_config = AppConfig::load().expect("Failed to load configuration from .env");
    let app_result = App::init(app_config).await;
    assert!(
        app_result.is_ok(),
        "Failed to initialize application: {:?}",
        app_result.err()
    );
    let app = app_result.unwrap();

    let db = &app.db;

    let test_block = Block {
        number: 12345,
        hash: "0x1234567890abcdef1234567890abcdef12345678".to_string(),
        parent_hash: "0x0987654321fedcba0987654321fedcba09876543".to_string(),
        timestamp: 1663224162,
        gas_used: 8000000,
        gas_limit: 15000000,
        transaction_count: 150,
        miner: Some("0xminer123".to_string()),
        difficulty: Some("1000000".to_string()),
        size_bytes: Some(50000),
        base_fee_per_gas: Some("20000000000".to_string()),
        extra_data: Some("0x".to_string()),
        state_root: Some("0xstate123".to_string()),
        nonce: Some("0x1234567890abcdef".to_string()),
        withdrawals_root: None,
        blob_gas_used: None,
        excess_blob_gas: None,
        withdrawal_count: None,
        slot: Some(4000000),
        proposer_index: Some(12345),
        epoch: Some(125000),
        slot_root: Some("0xslot123".to_string()),
        parent_root: Some("0xparent123".to_string()),
        beacon_deposit_count: Some(500000),
        graffiti: Some("test graffiti".to_string()),
        randao_reveal: Some("0xrandao123".to_string()),
        randao_mix: Some("0xmix123".to_string()),
    };

    let write_result = db.insert_block(&test_block).await;
    assert!(
        write_result.is_ok(),
        "Failed to insert block: {:?}",
        write_result.err()
    );

    let read_result = db.get_block_by_number(12345).await;
    assert!(
        read_result.is_ok(),
        "Failed to read block: {:?}",
        read_result.err()
    );
    let block_option = read_result.unwrap();
    assert!(block_option.is_some(), "Block not found");
    let retrieved_block = block_option.unwrap();
    assert_eq!(retrieved_block.number, 12345);
    assert_eq!(
        retrieved_block.hash,
        "0x1234567890abcdef1234567890abcdef12345678"
    );

    let test_transaction = Transaction {
        hash: "0xtx123456".to_string(),
        block_number: 12345,
        from_address: "0xfrom123".to_string(),
        to_address: Some("0xto123".to_string()),
        value: "1000000000000000000".to_string(),
        gas_used: 21000,
        gas_price: "20000000000".to_string(),
        status: 1,
        transaction_index: 0,
    };
    let tx_write_result = db.insert_transaction(&test_transaction).await;
    assert!(
        tx_write_result.is_ok(),
        "Failed to insert transaction: {:?}",
        tx_write_result.err()
    );

    let tx_read_result = db.get_transactions_by_block(12345).await;
    assert!(
        tx_read_result.is_ok(),
        "Failed to read transactions: {:?}",
        tx_read_result.err()
    );
    let transactions = tx_read_result.unwrap();
    assert_eq!(transactions.len(), 1);
    assert_eq!(transactions[0].hash, "0xtx123456");
}

#[tokio::test]
async fn test_api_endpoints() {
    std::env::set_var("DATABASE_URL", "sqlite:./data/test_api.db");
    let app_config = AppConfig::load().expect("Failed to load configuration from .env");
    let app_result = App::init(app_config).await;
    assert!(
        app_result.is_ok(),
        "Failed to initialize application: {:?}",
        app_result.err()
    );
    let app = app_result.unwrap();

    let _router = eth_indexer_rs::api::create_router(app.clone().into()).await;
}

#[tokio::test]
async fn test_rpc_connection_and_parsing() {
    std::env::set_var("DATABASE_URL", "sqlite:./data/test_indexer.db");
    let app_config = AppConfig::load().expect("Failed to load configuration from .env");
    let app_result = App::init(app_config).await;
    assert!(
        app_result.is_ok(),
        "Failed to initialize application: {:?}",
        app_result.err()
    );
    let app = app_result.unwrap();

    let rpc = &app.rpc;

    let latest_block_result = rpc.get_latest_block_number().await;
    if latest_block_result.is_ok() {
        let latest_block_number = latest_block_result.unwrap();
        assert!(latest_block_number > 0, "Block number must be positive");

        let block_result = rpc.get_block_by_number(latest_block_number).await;
        if block_result.is_ok() {
            let block_data = block_result.unwrap();
            if let Some(block) = block_data {
                assert!(block.number.is_some(), "Block must have a number");
                assert!(block.hash.is_some(), "Block must have a hash");
                assert!(
                    block.timestamp > ethers::types::U256::zero(),
                    "Block must have a timestamp"
                );
            }
        }
    }
}

#[tokio::test]
async fn test_beacon_connection_and_parsing() {
    std::env::set_var("DATABASE_URL", "sqlite:./data/test_indexer.db");
    let app_config = AppConfig::load().expect("Failed to load configuration from .env");
    let app_result = App::init(app_config).await;
    assert!(
        app_result.is_ok(),
        "Failed to initialize application: {:?}",
        app_result.err()
    );
    let app = app_result.unwrap();

    let beacon = &app.beacon;

    let connection_result = beacon.test_connection().await;
    if connection_result.is_ok() {
        let beacon_data_result = beacon.get_beacon_data_for_block(1000000).await;
        if beacon_data_result.is_ok() {
            let beacon_data = beacon_data_result.unwrap();
            assert!(beacon_data.is_object(), "Beacon data must be a JSON object");
        }
    }
}

#[tokio::test]
async fn test_full_integration_with_real_data() {
    std::env::set_var("DATABASE_URL", "sqlite:./data/test_integration.db");
    let app_config = AppConfig::load().expect("Failed to load configuration from .env");
    let app_result = App::init(app_config).await;
    assert!(
        app_result.is_ok(),
        "Failed to initialize application: {:?}",
        app_result.err()
    );
    let app = app_result.unwrap();

    let rpc = &app.rpc;
    let db = &app.db;

    let latest_block_result = rpc.get_latest_block_number().await;
    if latest_block_result.is_ok() {
        let latest_block_number = latest_block_result.unwrap();
        let test_block_number = if latest_block_number > 10 {
            latest_block_number - 10
        } else {
            latest_block_number
        };

        let block_result = rpc.get_block_by_number(test_block_number).await;
        if block_result.is_ok() {
            let block_data = block_result.unwrap();
            if let Some(eth_block) = block_data {
                let block = Block {
                    number: eth_block.number.unwrap_or_default().as_u64() as i64,
                    hash: format!("{:?}", eth_block.hash.unwrap_or_default()),
                    parent_hash: format!("{:?}", eth_block.parent_hash),
                    timestamp: eth_block.timestamp.as_u64() as i64,
                    gas_used: eth_block.gas_used.as_u64() as i64,
                    gas_limit: eth_block.gas_limit.as_u64() as i64,
                    transaction_count: eth_block.transactions.len() as i64,
                    miner: eth_block.author.map(|a| format!("{:?}", a)),
                    difficulty: Some(format!("{}", eth_block.difficulty)),
                    size_bytes: eth_block.size.map(|s| s.as_u64() as i64),
                    base_fee_per_gas: eth_block.base_fee_per_gas.map(|fee| format!("{}", fee)),
                    extra_data: Some(format!("{:?}", eth_block.extra_data)),
                    state_root: Some(format!("{:?}", eth_block.state_root)),
                    nonce: eth_block.nonce.map(|n| format!("{:?}", n)),
                    withdrawals_root: None,
                    blob_gas_used: None,
                    excess_blob_gas: None,
                    withdrawal_count: None,
                    slot: None,
                    proposer_index: None,
                    epoch: None,
                    slot_root: None,
                    parent_root: None,
                    beacon_deposit_count: None,
                    graffiti: None,
                    randao_reveal: None,
                    randao_mix: None,
                };

                let save_result = db.insert_block(&block).await;
                assert!(
                    save_result.is_ok(),
                    "Failed to save real block: {:?}",
                    save_result.err()
                );

                let retrieved_result = db.get_block_by_number(block.number).await;
                assert!(retrieved_result.is_ok(), "Failed to retrieve saved block");
                let retrieved_block = retrieved_result.unwrap();
                assert!(retrieved_block.is_some(), "Saved block not found");
                let saved_block = retrieved_block.unwrap();
                assert_eq!(saved_block.hash, block.hash);
                assert_eq!(saved_block.number, block.number);
            }
        }
    }
}
