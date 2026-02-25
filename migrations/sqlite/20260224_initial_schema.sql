-- Initial schema for drakeify database (SQLite)

-- Secrets table - stores encrypted secrets and API keys
CREATE TABLE IF NOT EXISTS secrets (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Plugin configurations table - stores plugin-specific settings
CREATE TABLE IF NOT EXISTS plugin_configs (
    plugin_name TEXT PRIMARY KEY NOT NULL,
    config TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Create indexes for better query performance
CREATE INDEX IF NOT EXISTS idx_secrets_updated_at ON secrets(updated_at);
CREATE INDEX IF NOT EXISTS idx_plugin_configs_updated_at ON plugin_configs(updated_at);

