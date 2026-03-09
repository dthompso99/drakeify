-- Consolidated Initial Schema for Drakeify (PostgreSQL)
-- This combines all previous migrations into a single schema

-- ============================================================================
-- SECRETS TABLE
-- ============================================================================
-- Stores encrypted secrets and API keys
CREATE TABLE IF NOT EXISTS secrets (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_secrets_updated_at ON secrets(updated_at);

-- ============================================================================
-- PLUGIN CONFIGURATIONS TABLE
-- ============================================================================
-- Stores plugin-specific settings
CREATE TABLE IF NOT EXISTS plugin_configs (
    plugin_name TEXT PRIMARY KEY NOT NULL,
    config TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_plugin_configs_updated_at ON plugin_configs(updated_at);

-- ============================================================================
-- SESSIONS TABLE
-- ============================================================================
-- Stores conversation sessions tied to accounts
CREATE TABLE IF NOT EXISTS sessions (
    session_id TEXT PRIMARY KEY NOT NULL,
    account_id TEXT NOT NULL,
    messages TEXT NOT NULL,  -- JSON serialized Vec<OllamaMessage>
    metadata TEXT NOT NULL,  -- JSON serialized SessionMetadata
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_sessions_account_id ON sessions(account_id);
CREATE INDEX IF NOT EXISTS idx_sessions_updated_at ON sessions(updated_at);
CREATE INDEX IF NOT EXISTS idx_sessions_account_updated ON sessions(account_id, updated_at DESC);

-- ============================================================================
-- SCHEDULED JOBS TABLE
-- ============================================================================
-- Stores tasks to be executed at a specific time
CREATE TABLE IF NOT EXISTS scheduled_jobs (
    id SERIAL PRIMARY KEY,
    account_id TEXT NOT NULL,
    session_id TEXT,  -- Optional: session to load context from
    prompt TEXT NOT NULL,  -- The task to execute
    context TEXT,  -- JSON: additional context (tool configs, etc.)
    run_at TIMESTAMPTZ NOT NULL,  -- When to run the job
    status TEXT NOT NULL DEFAULT 'pending',  -- pending, running, completed, failed
    locked_at TIMESTAMPTZ,  -- When the job was claimed
    locked_by TEXT,  -- Pod/instance identifier that claimed the job
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TIMESTAMPTZ,
    result TEXT,  -- JSON: result of execution
    error TEXT  -- Error message if failed
);

CREATE INDEX IF NOT EXISTS idx_scheduled_jobs_pending ON scheduled_jobs(run_at) WHERE status = 'pending';
CREATE INDEX IF NOT EXISTS idx_scheduled_jobs_account ON scheduled_jobs(account_id);
CREATE INDEX IF NOT EXISTS idx_scheduled_jobs_session ON scheduled_jobs(session_id);
CREATE INDEX IF NOT EXISTS idx_scheduled_jobs_status ON scheduled_jobs(status);

-- ============================================================================
-- DOCUMENT STORE TABLE
-- ============================================================================
-- Flexible key-value store for plugins and tools
-- Namespaced by plugin/tool to avoid key collisions
CREATE TABLE IF NOT EXISTS documents (
    id SERIAL PRIMARY KEY,
    namespace TEXT NOT NULL,           -- e.g., "plugin:zulip", "tool:brave_search"
    key TEXT NOT NULL,                 -- Document key within namespace
    value TEXT NOT NULL,               -- JSON or plain text value
    account_id TEXT NOT NULL,          -- Account this document belongs to
    metadata TEXT DEFAULT '{}',        -- Additional metadata (JSON string)
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    
    -- Composite unique constraint: one key per namespace per account
    UNIQUE(namespace, key, account_id)
);

CREATE INDEX IF NOT EXISTS idx_documents_namespace_account ON documents(namespace, account_id);
CREATE INDEX IF NOT EXISTS idx_documents_account ON documents(account_id);
CREATE INDEX IF NOT EXISTS idx_documents_namespace_key ON documents(namespace, key);

-- ============================================================================
-- LLM CONFIGURATIONS TABLE
-- ============================================================================
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
    account_id TEXT,                              -- API key/account ID for LLM provider (e.g., OpenAI key)
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_llm_configs_enabled ON llm_configs(enabled);
CREATE INDEX IF NOT EXISTS idx_llm_configs_priority ON llm_configs(priority DESC);

-- ============================================================================
-- GLOBAL CONFIGURATION TABLE
-- ============================================================================
-- Stores system-wide settings (key-value pairs)
CREATE TABLE IF NOT EXISTS global_config (
    key TEXT PRIMARY KEY,                         -- e.g., "default_llm_id", "routing_strategy"
    value TEXT NOT NULL,                          -- Configuration value (can be JSON)
    description TEXT,                             -- Human-readable description
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

