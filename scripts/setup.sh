#!/bin/bash
set -e

echo "🚀 Setting up ETH Indexer RS..."

# Check dependencies
if ! command -v cargo &> /dev/null; then
    echo "❌ Cargo not found. Please install Rust first."
    exit 1
fi

# Create .env file if it doesn't exist
if [ ! -f .env ]; then
    echo "📝 Creating .env file..."
    cat > .env << EOF
# ETH Indexer RS Configuration

# Database connection string
DATABASE_URL=sqlite:./data/indexer.db

# Ethereum RPC URL (replace with your own endpoint)
ETH_RPC_URL=https://mainnet.infura.io/v3/your-infura-key

# API server port
API_PORT=3000

# Starting block number (optional, defaults to 0)
# START_BLOCK=15000000

# Maximum concurrent RPC requests
MAX_CONCURRENT_REQUESTS=5

# Number of blocks to process in a batch
BLOCKS_PER_BATCH=10

# Log level: trace, debug, info, warn, error
LOG_LEVEL=info
EOF
    echo "✅ Created .env file. Please edit it with your Ethereum RPC endpoint."
fi

# Create data directory
mkdir -p data

# Build the project
echo "🔨 Building project..."
cargo build

echo "✅ Setup complete! Run './scripts/start.sh' to start the application."
