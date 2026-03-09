-- Add account_id column to llm_configs table
-- This is the API key/account ID to pass to the LLM provider (e.g., OpenAI API key)

ALTER TABLE llm_configs ADD COLUMN IF NOT EXISTS account_id TEXT;

