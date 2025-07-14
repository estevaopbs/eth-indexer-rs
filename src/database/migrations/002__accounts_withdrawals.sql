-- Migration 002: Accounts and Staking Data
-- Creates tables for account tracking and Ethereum staking/withdrawal data
-- This represents post-Shanghai fork functionality

-- ============================================================================
-- ACCOUNTS TABLE - Track account balances and activity
-- ============================================================================
CREATE TABLE IF NOT EXISTS accounts (
    address TEXT PRIMARY KEY,                      -- Account address
    balance TEXT NOT NULL,                         -- Current balance in wei
    transaction_count INTEGER DEFAULT 0,           -- Number of transactions
    first_seen_block INTEGER,                      -- First block this account appeared
    last_seen_block INTEGER,                       -- Last block this account was active
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Create indexes for accounts table
CREATE INDEX IF NOT EXISTS idx_accounts_balance ON accounts(balance);
CREATE INDEX IF NOT EXISTS idx_accounts_first_seen ON accounts(first_seen_block);
CREATE INDEX IF NOT EXISTS idx_accounts_last_seen ON accounts(last_seen_block);

-- ============================================================================
-- WITHDRAWALS TABLE - Ethereum Shanghai fork validator withdrawals
-- ============================================================================
CREATE TABLE IF NOT EXISTS withdrawals (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    block_number INTEGER NOT NULL,                 -- Block containing withdrawal
    withdrawal_index INTEGER NOT NULL,             -- Index within the block
    validator_index INTEGER NOT NULL,              -- Validator index on beacon chain
    address TEXT NOT NULL,                         -- Withdrawal recipient address
    amount TEXT NOT NULL,                          -- Amount in Gwei as string
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (block_number) REFERENCES blocks (number)
);

-- Create indexes for withdrawals table
CREATE INDEX IF NOT EXISTS idx_withdrawals_block ON withdrawals(block_number);
CREATE INDEX IF NOT EXISTS idx_withdrawals_address ON withdrawals(address);
CREATE INDEX IF NOT EXISTS idx_withdrawals_validator ON withdrawals(validator_index);
CREATE INDEX IF NOT EXISTS idx_withdrawals_index ON withdrawals(withdrawal_index);

-- Add unique constraint for withdrawals to support ON CONFLICT clause
-- This ensures that each withdrawal is unique by block_number and withdrawal_index
CREATE UNIQUE INDEX IF NOT EXISTS idx_withdrawals_block_withdrawal 
ON withdrawals(block_number, withdrawal_index);
