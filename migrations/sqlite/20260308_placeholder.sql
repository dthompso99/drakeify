-- Scheduled Jobs table - stores tasks to be executed at a specific time

CREATE TABLE IF NOT EXISTS scheduled_jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id TEXT NOT NULL,
    session_id TEXT,  -- Optional: session to load context from
    prompt TEXT NOT NULL,  -- The task to execute
    context TEXT,  -- JSON: additional context (tool configs, etc.)
    run_at TEXT NOT NULL,  -- ISO 8601 timestamp
    status TEXT NOT NULL DEFAULT 'pending',  -- pending, running, completed, failed
    locked_at TEXT,  -- When the job was claimed
    locked_by TEXT,  -- Pod/instance identifier that claimed the job
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    completed_at TEXT,
    result TEXT,  -- JSON: result of execution
    error TEXT  -- Error message if failed
);

-- Create indexes for better query performance
CREATE INDEX IF NOT EXISTS idx_scheduled_jobs_pending ON scheduled_jobs(run_at) WHERE status = 'pending';
CREATE INDEX IF NOT EXISTS idx_scheduled_jobs_account ON scheduled_jobs(account_id);
CREATE INDEX IF NOT EXISTS idx_scheduled_jobs_session ON scheduled_jobs(session_id);
CREATE INDEX IF NOT EXISTS idx_scheduled_jobs_status ON scheduled_jobs(status);

