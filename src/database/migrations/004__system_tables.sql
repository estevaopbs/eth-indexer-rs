-- Migration 004: System Tables and Cache
-- Creates system tables for configuration and historical data caching
-- This represents internal system state and performance optimization

-- ============================================================================
-- START BLOCK CACHE TABLE - Consolidates config and historical cache
-- ============================================================================
CREATE TABLE IF NOT EXISTS start_block_cache (
    start_block INTEGER PRIMARY KEY,
    total_transactions_before INTEGER NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Create trigger to update updated_at timestamp
CREATE TRIGGER IF NOT EXISTS update_start_block_cache_updated_at 
    AFTER UPDATE ON start_block_cache
    FOR EACH ROW
BEGIN
    UPDATE start_block_cache SET updated_at = CURRENT_TIMESTAMP WHERE start_block = NEW.start_block;
END;
