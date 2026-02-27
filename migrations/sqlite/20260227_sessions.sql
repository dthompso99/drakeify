-- Sessions table - stores conversation sessions tied to accounts

CREATE TABLE IF NOT EXISTS sessions (
    session_id TEXT PRIMARY KEY NOT NULL,
    account_id TEXT NOT NULL,
    messages TEXT NOT NULL,  -- JSON serialized Vec<OllamaMessage>
    metadata TEXT NOT NULL,  -- JSON serialized SessionMetadata
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Create indexes for better query performance
CREATE INDEX IF NOT EXISTS idx_sessions_account_id ON sessions(account_id);
CREATE INDEX IF NOT EXISTS idx_sessions_updated_at ON sessions(updated_at);
CREATE INDEX IF NOT EXISTS idx_sessions_account_updated ON sessions(account_id, updated_at DESC);

