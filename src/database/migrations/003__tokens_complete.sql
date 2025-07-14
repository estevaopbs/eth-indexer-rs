-- Migration 003: Complete Token System
-- Creates tables for ERC-20/ERC-721/ERC-1155 token transfers, token metadata and account balances
-- This represents the complete token tracking system

-- ============================================================================
-- TOKEN TRANSFERS TABLE - ERC-20/ERC-721/ERC-1155 token movements
-- ============================================================================
CREATE TABLE IF NOT EXISTS token_transfers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    transaction_hash TEXT NOT NULL,                -- Transaction containing the transfer
    block_number INTEGER NOT NULL,                 -- Block number reference
    token_address TEXT NOT NULL,                   -- Contract address of the token
    from_address TEXT NOT NULL,                    -- Sender address
    to_address TEXT NOT NULL,                      -- Recipient address
    amount TEXT NOT NULL,                          -- Amount transferred (as string for precision)
    token_type TEXT DEFAULT 'ERC20',               -- Token standard (ERC20, ERC721, ERC1155)
    token_id TEXT,                                 -- Token ID for NFTs (ERC721/ERC1155)
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (transaction_hash) REFERENCES transactions (hash),
    FOREIGN KEY (block_number) REFERENCES blocks (number)
);

-- Create indexes for token transfers table
CREATE INDEX IF NOT EXISTS idx_token_transfers_tx ON token_transfers(transaction_hash);
CREATE INDEX IF NOT EXISTS idx_token_transfers_block ON token_transfers(block_number);
CREATE INDEX IF NOT EXISTS idx_token_transfers_token ON token_transfers(token_address);
CREATE INDEX IF NOT EXISTS idx_token_transfers_from ON token_transfers(from_address);
CREATE INDEX IF NOT EXISTS idx_token_transfers_to ON token_transfers(to_address);
CREATE INDEX IF NOT EXISTS idx_token_transfers_type ON token_transfers(token_type);

-- ============================================================================
-- TOKENS TABLE - Metadata for ERC-20/ERC-721/ERC-1155 tokens
-- ============================================================================
CREATE TABLE IF NOT EXISTS tokens (
    address TEXT PRIMARY KEY NOT NULL,             -- Contract address of the token
    name TEXT,                                     -- Token name (from name() call)
    symbol TEXT,                                   -- Token symbol (from symbol() call)
    decimals INTEGER,                              -- Token decimals (from decimals() call)
    token_type TEXT NOT NULL DEFAULT 'ERC20',      -- Token standard (ERC20, ERC721, ERC1155)
    first_seen_block INTEGER NOT NULL,            -- First block where token was seen
    last_seen_block INTEGER NOT NULL,             -- Last block where token was seen
    total_transfers INTEGER DEFAULT 0,            -- Total number of transfers
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- ============================================================================
-- TOKEN_BALANCES TABLE - Account token balances (NO FOREIGN KEYS)
-- ============================================================================
-- Note: This table deliberately has no foreign key constraints to avoid
-- insertion issues when token balances exist for accounts not yet indexed
CREATE TABLE IF NOT EXISTS token_balances (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_address TEXT NOT NULL,                 -- Account holding the tokens
    token_address TEXT NOT NULL,                   -- Token contract address
    balance TEXT NOT NULL,                         -- Current balance (as string for precision)
    block_number INTEGER NOT NULL,                -- Block number when balance was first recorded
    last_updated_block INTEGER NOT NULL,          -- Block number when balance was last updated
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    
    -- Unique constraint for account-token pairs
    UNIQUE(account_address, token_address)
);

-- ============================================================================
-- INDEXES for performance
-- ============================================================================

-- Tokens table indexes
CREATE INDEX IF NOT EXISTS idx_tokens_symbol ON tokens(symbol);
CREATE INDEX IF NOT EXISTS idx_tokens_type ON tokens(token_type);
CREATE INDEX IF NOT EXISTS idx_tokens_first_seen ON tokens(first_seen_block);
CREATE INDEX IF NOT EXISTS idx_tokens_last_seen ON tokens(last_seen_block);

-- Token balances table indexes
CREATE INDEX IF NOT EXISTS idx_token_balances_account ON token_balances(account_address);
CREATE INDEX IF NOT EXISTS idx_token_balances_token ON token_balances(token_address);
CREATE INDEX IF NOT EXISTS idx_token_balances_block ON token_balances(block_number);
CREATE INDEX IF NOT EXISTS idx_token_balances_updated ON token_balances(last_updated_block);

-- ============================================================================
-- TRIGGERS for automatic timestamp updates
-- ============================================================================

-- Update tokens updated_at timestamp
CREATE TRIGGER IF NOT EXISTS update_tokens_updated_at
    AFTER UPDATE ON tokens
    BEGIN
        UPDATE tokens SET updated_at = CURRENT_TIMESTAMP WHERE address = NEW.address;
    END;

-- Update token_balances updated_at timestamp
CREATE TRIGGER IF NOT EXISTS update_token_balances_updated_at
    AFTER UPDATE ON token_balances
    BEGIN
        UPDATE token_balances SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
    END;
