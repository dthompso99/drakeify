-- Document Store: Flexible key-value store for plugins and tools
-- Namespaced by plugin/tool to avoid key collisions

CREATE TABLE IF NOT EXISTS documents (
    id SERIAL PRIMARY KEY,
    namespace TEXT NOT NULL,           -- e.g., "plugin:zulip", "tool:brave_search"
    key TEXT NOT NULL,                 -- Document key within namespace
    value TEXT NOT NULL,               -- JSON or plain text value
    account_id TEXT NOT NULL,          -- Account this document belongs to
    metadata JSONB DEFAULT '{}',       -- Additional metadata
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Composite unique constraint: one key per namespace per account
    UNIQUE(namespace, key, account_id)
);

-- Index for efficient namespace + account queries
CREATE INDEX IF NOT EXISTS idx_documents_namespace_account 
    ON documents(namespace, account_id);

-- Index for efficient account-wide queries
CREATE INDEX IF NOT EXISTS idx_documents_account 
    ON documents(account_id);

-- Index for efficient key lookups within namespace
CREATE INDEX IF NOT EXISTS idx_documents_namespace_key 
    ON documents(namespace, key);

