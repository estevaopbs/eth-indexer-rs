-- Migration 003: Token and DeFi Data
-- Creates tables for ERC-20 token transfers and DeFi activity tracking
-- This represents application layer data on top of the execution layer

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
