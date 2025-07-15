# ETH Indexer RS

ETH Indexer RS is a blockchain indexer for Ethereum networks, developed in Rust. It collects, stores, and serves data on blocks, transactions, accounts, and events from Ethereum, offering an API and a web interface for real-time querying and visualization.

## Features

- Real-time indexing of Ethereum blocks, transactions, accounts, and logs.
- RESTful API for querying indexed data.
- Modern web interface with dashboards, search, and detailed views for blocks, transactions, and accounts.
- Support for historical data and network statistics.
- Visualization of gas usage and transactions per block charts.
- Pagination and filters for large data volumes.
- Support for ERC-20 tokens and token transfers.

## How to Run

1. **Prerequisites**  
    - [Rust](https://www.rust-lang.org/tools/install)
    - [Node.js](https://nodejs.org/) (optional, for frontend development)
    - SQLite3

2. **Configuration**  
    - Adjust `.env` as needed.
    - Install dependencies:
      ```sh
      cargo build
      ```

3. **Execution**  
    - Start the indexer:
      ```sh
      cargo run
      ```
    - Access the web interface at `http://localhost:3000`

4. **Testing**
    ```sh
    cargo test
    ```

## Main API Endpoints

- GET /api/blocks — List indexed blocks
- GET /api/blocks/{number} — Block details
- GET /api/transactions — List transactions
- GET /api/transactions/{hash} — Transaction details
- GET /api/accounts — List accounts
- GET /api/accounts/{address} — Account details
- GET /api/stats — Indexer statistics

## Frontend

The static frontend is located in the `static` directory and can be served directly by the Rust backend. It includes pages for the dashboard, blocks, transactions, and accounts.

## Database

The project uses SQLite by default, with SQL migrations in the `migrations` directory.

## License

MIT License. See the [LICENSE](LICENSE) file for more details.
