-- Migration 001: Core Blockchain Data
-- Creates the fundamental tables for blocks, transactions, and logs
-- This represents the core Ethereum execution layer data

-- ============================================================================
-- BLOCKS TABLE - Core execution layer block data
-- ============================================================================
CREATE TABLE IF NOT EXISTS blocks (
    -- Core block identifiers and metadata
    number INTEGER PRIMARY KEY,                    -- Block Height/Number ✅ STORED
    hash TEXT NOT NULL UNIQUE,                     -- Hash ✅ STORED
    parent_hash TEXT NOT NULL,                     -- Parent Hash ✅ STORED
    timestamp INTEGER NOT NULL,                    -- Timestamp ✅ STORED
    
    -- Gas and transaction data
    gas_used INTEGER NOT NULL,                     -- Gas Used ✅ STORED
    gas_limit INTEGER NOT NULL,                    -- Gas Limit ✅ STORED
    transaction_count INTEGER NOT NULL,            -- Transactions (count) ✅ STORED
    
    -- Block producer and fees (EIP-1559)
    miner TEXT,                                    -- Fee Recipient ✅ STORED
    base_fee_per_gas TEXT,                         -- Base Fee Per Gas ✅ STORED
    
    -- Block structure and state
    total_difficulty TEXT,                         -- Total Difficulty ✅ STORED
    size_bytes INTEGER,                            -- Size (bytes) ✅ STORED
    extra_data TEXT,                               -- Extra Data ✅ STORED
    state_root TEXT,                               -- StateRoot ✅ STORED
    nonce TEXT,                                    -- Nonce ✅ STORED
    
    -- Post-Shanghai (Withdrawals) fields
    withdrawals_root TEXT,                         -- WithdrawalsRoot ✅ STORED
    withdrawal_count INTEGER DEFAULT 0,           -- Withdrawals (count) ✅ STORED
    
    -- EIP-4844 (Dencun/Blob) fields
    blob_gas_used INTEGER,                         -- Blob Gas Used ✅ STORED
    excess_blob_gas INTEGER,                       -- Excess blob gas ✅ STORED
    
    -- Beacon Chain fields (requires separate API connection)
    slot INTEGER,                                  -- Beacon chain slot ✅ STORED
    proposer_index INTEGER,                        -- Validator proposer index ✅ STORED
    epoch INTEGER,                                 -- Beacon chain epoch ✅ STORED
    slot_root TEXT,                                -- Slot root hash ✅ STORED
    parent_root TEXT,                              -- Parent root hash ✅ STORED
    beacon_deposit_count INTEGER,                  -- Beacon chain deposit count ✅ STORED
    graffiti TEXT,                                 -- Proposer graffiti ✅ STORED
    randao_reveal TEXT,                            -- Randao reveal signature ✅ STORED
    randao_mix TEXT,                               -- Block randomness ✅ STORED
    
    -- Metadata
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for blocks table
CREATE INDEX IF NOT EXISTS idx_blocks_hash ON blocks(hash);
CREATE INDEX IF NOT EXISTS idx_blocks_miner ON blocks(miner);
CREATE INDEX IF NOT EXISTS idx_blocks_timestamp ON blocks(timestamp);
CREATE INDEX IF NOT EXISTS idx_blocks_base_fee ON blocks(base_fee_per_gas);
CREATE INDEX IF NOT EXISTS idx_blocks_slot ON blocks(slot);

-- ============================================================================
-- TRANSACTIONS TABLE - Individual transaction data
-- ============================================================================
CREATE TABLE IF NOT EXISTS transactions (
    hash TEXT PRIMARY KEY,                         -- Transaction hash
    block_number INTEGER NOT NULL,                 -- Block number reference
    from_address TEXT NOT NULL,                    -- Sender address
    to_address TEXT,                               -- Recipient address (null for contract creation)
    value TEXT NOT NULL,                           -- Transaction value in wei
    gas_used INTEGER NOT NULL,                     -- Gas used by transaction
    gas_price TEXT NOT NULL,                       -- Gas price in wei
    status INTEGER NOT NULL,                       -- Transaction status (1=success, 0=failure)
    transaction_index INTEGER NOT NULL,            -- Index within block
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (block_number) REFERENCES blocks (number)
);

-- Create indexes for transactions table
CREATE INDEX IF NOT EXISTS idx_transactions_block ON transactions(block_number);
CREATE INDEX IF NOT EXISTS idx_transactions_from ON transactions(from_address);
CREATE INDEX IF NOT EXISTS idx_transactions_to ON transactions(to_address);
CREATE INDEX IF NOT EXISTS idx_transactions_status ON transactions(status);

-- ============================================================================
-- LOGS TABLE - Event logs from smart contracts
-- ============================================================================
CREATE TABLE IF NOT EXISTS logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    transaction_hash TEXT NOT NULL,
    block_number INTEGER NOT NULL,
    address TEXT NOT NULL,                         -- Contract address that emitted the log
    topic0 TEXT,                                   -- Event signature hash
    topic1 TEXT,                                   -- First indexed parameter
    topic2 TEXT,                                   -- Second indexed parameter  
    topic3 TEXT,                                   -- Third indexed parameter
    data TEXT,                                     -- Non-indexed event data
    log_index INTEGER NOT NULL,                    -- Index within transaction
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (transaction_hash) REFERENCES transactions (hash),
    FOREIGN KEY (block_number) REFERENCES blocks (number)
);

-- Create indexes for logs table
CREATE INDEX IF NOT EXISTS idx_logs_transaction ON logs(transaction_hash);
CREATE INDEX IF NOT EXISTS idx_logs_block ON logs(block_number);
CREATE INDEX IF NOT EXISTS idx_logs_address ON logs(address);
CREATE INDEX IF NOT EXISTS idx_logs_topic0 ON logs(topic0);
CREATE INDEX IF NOT EXISTS idx_logs_topic1 ON logs(topic1);
