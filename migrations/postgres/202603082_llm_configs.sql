-- LLM Configurations Table
-- Stores multiple LLM server/model configurations for dynamic routing

CREATE TABLE IF NOT EXISTS llm_configs (
    id TEXT PRIMARY KEY,                          -- e.g., "default", "vision", "code-expert"
    name TEXT NOT NULL,                           -- Human-readable name
    host TEXT NOT NULL,                           -- e.g., "http://localhost:11434"
    endpoint TEXT NOT NULL,                       -- e.g., "/api/chat" or "/v1/chat/completions"
    model TEXT NOT NULL,                          -- e.g., "llama3.1:latest"
    context_size INTEGER NOT NULL DEFAULT 32768,  -- Context window size
    timeout_secs INTEGER NOT NULL DEFAULT 900,    -- Request timeout in seconds
    capabilities TEXT NOT NULL DEFAULT '[]',      -- JSON array: ["text", "vision", "code"]
    priority INTEGER NOT NULL DEFAULT 0,          -- Higher = preferred when multiple match
    enabled BOOLEAN NOT NULL DEFAULT TRUE,        -- Whether this config is active
    metadata TEXT NOT NULL DEFAULT '{}',          -- JSON object for extra config
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for efficient queries
CREATE INDEX IF NOT EXISTS idx_llm_configs_enabled ON llm_configs(enabled);
CREATE INDEX IF NOT EXISTS idx_llm_configs_priority ON llm_configs(priority DESC);

-- Global Configuration Table
-- Stores system-wide settings (key-value pairs)

CREATE TABLE IF NOT EXISTS global_config (
    key TEXT PRIMARY KEY,                         -- e.g., "default_llm_id", "routing_strategy"
    value TEXT NOT NULL,                          -- Configuration value (can be JSON)
    description TEXT,                             -- Human-readable description
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

