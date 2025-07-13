#!/bin/bash
set -e

echo "🚀 Starting ETH Indexer RS..."

# Load environment variables
if [ -f .env ]; then
    export $(grep -v '^#' .env | xargs)
fi

# Start the application
cargo run

# Handle interruptions
trap "echo '⛔ Stopping ETH Indexer RS'; exit" INT TERM
