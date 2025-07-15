-- Migration 001: Core Blockchain Data

-- BLOCKS TABLE
CREATE TABLE IF NOT EXISTS blocks (
    number INTEGER PRIMARY KEY,
    hash TEXT NOT NULL UNIQUE,
    parent_hash TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    gas_used INTEGER NOT NULL,
    gas_limit INTEGER NOT NULL,
    transaction_count INTEGER NOT NULL,
    miner TEXT,
    base_fee_per_gas TEXT,
    total_difficulty TEXT,
    size_bytes INTEGER,
    extra_data TEXT,
    state_root TEXT,
    nonce TEXT,
    withdrawals_root TEXT,
    withdrawal_count INTEGER DEFAULT 0,           
    
    -- EIP-4844 (Dencun/Blob) fields
    blob_gas_used INTEGER,                         
    excess_blob_gas INTEGER,                       
    
    -- Beacon Chain fields (requires separate API connection)
    slot INTEGER,                                  
    proposer_index INTEGER,                        
    epoch INTEGER,                                 
    slot_root TEXT,                                
    parent_root TEXT,                              
    beacon_deposit_count INTEGER,                  
    graffiti TEXT,                                 
    randao_reveal TEXT,                            
    randao_mix TEXT,                               
    
    -- Metadata
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for blocks table
CREATE INDEX IF NOT EXISTS idx_blocks_hash ON blocks(hash);
CREATE INDEX IF NOT EXISTS idx_blocks_miner ON blocks(miner);
CREATE INDEX IF NOT EXISTS idx_blocks_timestamp ON blocks(timestamp);
CREATE INDEX IF NOT EXISTS idx_blocks_base_fee ON blocks(base_fee_per_gas);
CREATE INDEX IF NOT EXISTS idx_blocks_slot ON blocks(slot);

-- TRANSACTIONS TABLE - Individual transaction data
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

-- LOGS TABLE - Event logs from smart contracts
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
