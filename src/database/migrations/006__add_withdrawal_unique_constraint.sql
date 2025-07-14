-- Add unique constraint for withdrawals to support ON CONFLICT clause
-- This ensures that each withdrawal is unique by block_number and withdrawal_index
CREATE UNIQUE INDEX IF NOT EXISTS idx_withdrawals_block_withdrawal 
ON withdrawals(block_number, withdrawal_index);
