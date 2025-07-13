#!/bin/bash
# Development build and run script for eth-indexer-rs

set -e

# Load local environment
if [ -f ".env.local" ]; then
    export $(cat .env.local | xargs)
fi

echo "🔧 Building ETH Indexer RS..."

# Build the project with SQLX offline mode
export SQLX_OFFLINE=true
cargo build "$@"

echo "✅ Build completed successfully!"

# If --run flag is provided, also run the project
if [[ " $* " == *" --run "* ]]; then
    echo "🚀 Starting ETH Indexer RS..."
    cargo run
fi
