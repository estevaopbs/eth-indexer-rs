-- Add historical transaction cache table
CREATE TABLE IF NOT EXISTS historical_transaction_cache (
    block_number INTEGER PRIMARY KEY,
    total_transactions_before INTEGER NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Create index for faster lookups
CREATE INDEX IF NOT EXISTS idx_historical_cache_block ON historical_transaction_cache(block_number);
