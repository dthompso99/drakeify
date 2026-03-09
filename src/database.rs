use anyhow::{Context, Result};
use sqlx::migrate::MigrateDatabase;
use sqlx::{Pool, Sqlite, Postgres};
use tracing::{info, debug};

/// Database abstraction supporting both SQLite and PostgreSQL
#[derive(Clone)]
pub enum Database {
    Sqlite(Pool<Sqlite>),
    Postgres(Pool<Postgres>),
}

impl Database {
    /// Connect to a database using the provided URL
    /// 
    /// Supports:
    /// - SQLite: `sqlite://path/to/db.db` or `sqlite::memory:`
    /// - PostgreSQL: `postgres://user:pass@host/db` or `postgresql://...`
    pub async fn connect(database_url: &str) -> Result<Self> {
        info!("🗄️  Connecting to database: {}", Self::sanitize_url(database_url));
        
        if database_url.starts_with("sqlite:") {
            Self::connect_sqlite(database_url).await
        } else if database_url.starts_with("postgres:") || database_url.starts_with("postgresql:") {
            Self::connect_postgres(database_url).await
        } else {
            Err(anyhow::anyhow!(
                "Unsupported database URL. Must start with 'sqlite:', 'postgres:', or 'postgresql:'"
            ))
        }
    }
    
    /// Connect to SQLite database
    async fn connect_sqlite(database_url: &str) -> Result<Self> {
        // Create database file if it doesn't exist
        if !Sqlite::database_exists(database_url).await.unwrap_or(false) {
            info!("Creating SQLite database...");
            Sqlite::create_database(database_url).await?;
        }
        
        let pool = sqlx::SqlitePool::connect(database_url)
            .await
            .context("Failed to connect to SQLite database")?;
        
        info!("✓ Connected to SQLite database");
        Ok(Database::Sqlite(pool))
    }
    
    /// Connect to PostgreSQL database
    async fn connect_postgres(database_url: &str) -> Result<Self> {
        let pool = sqlx::PgPool::connect(database_url)
            .await
            .context("Failed to connect to PostgreSQL database")?;
        
        info!("✓ Connected to PostgreSQL database");
        Ok(Database::Postgres(pool))
    }
    
    /// Run database migrations
    pub async fn migrate(&self) -> Result<()> {
        info!("Running database migrations...");
        
        match self {
            Database::Sqlite(pool) => {
                sqlx::migrate!("./migrations/sqlite")
                    .run(pool)
                    .await
                    .context("Failed to run SQLite migrations")?;
            }
            Database::Postgres(pool) => {
                sqlx::migrate!("./migrations/postgres")
                    .run(pool)
                    .await
                    .context("Failed to run PostgreSQL migrations")?;
            }
        }
        
        info!("✓ Database migrations complete");
        Ok(())
    }
    
    /// Get a secret value by key
    pub async fn get_secret(&self, key: &str) -> Result<Option<String>> {
        debug!("Getting secret: {}", key);
        
        let result = match self {
            Database::Sqlite(pool) => {
                sqlx::query_scalar::<_, String>("SELECT value FROM secrets WHERE key = ?")
                    .bind(key)
                    .fetch_optional(pool)
                    .await?
            }
            Database::Postgres(pool) => {
                sqlx::query_scalar::<_, String>("SELECT value FROM secrets WHERE key = $1")
                    .bind(key)
                    .fetch_optional(pool)
                    .await?
            }
        };
        
        Ok(result)
    }
    
    /// Set a secret value
    pub async fn set_secret(&self, key: &str, value: &str) -> Result<()> {
        debug!("Setting secret: {}", key);
        
        match self {
            Database::Sqlite(pool) => {
                sqlx::query("INSERT OR REPLACE INTO secrets (key, value, updated_at) VALUES (?, ?, datetime('now'))")
                    .bind(key)
                    .bind(value)
                    .execute(pool)
                    .await?;
            }
            Database::Postgres(pool) => {
                sqlx::query("INSERT INTO secrets (key, value, updated_at) VALUES ($1, $2, NOW()) ON CONFLICT (key) DO UPDATE SET value = $2, updated_at = NOW()")
                    .bind(key)
                    .bind(value)
                    .execute(pool)
                    .await?;
            }
        }
        
        Ok(())
    }
    
    /// Delete a secret
    pub async fn delete_secret(&self, key: &str) -> Result<bool> {
        debug!("Deleting secret: {}", key);
        
        let rows_affected = match self {
            Database::Sqlite(pool) => {
                sqlx::query("DELETE FROM secrets WHERE key = ?")
                    .bind(key)
                    .execute(pool)
                    .await?
                    .rows_affected()
            }
            Database::Postgres(pool) => {
                sqlx::query("DELETE FROM secrets WHERE key = $1")
                    .bind(key)
                    .execute(pool)
                    .await?
                    .rows_affected()
            }
        };
        
        Ok(rows_affected > 0)
    }

    /// Get plugin configuration
    pub async fn get_plugin_config(&self, plugin_name: &str) -> Result<Option<String>> {
        debug!("Getting plugin config: {}", plugin_name);

        let result = match self {
            Database::Sqlite(pool) => {
                sqlx::query_scalar::<_, String>("SELECT config FROM plugin_configs WHERE plugin_name = ?")
                    .bind(plugin_name)
                    .fetch_optional(pool)
                    .await?
            }
            Database::Postgres(pool) => {
                sqlx::query_scalar::<_, String>("SELECT config FROM plugin_configs WHERE plugin_name = $1")
                    .bind(plugin_name)
                    .fetch_optional(pool)
                    .await?
            }
        };

        Ok(result)
    }

    /// Set plugin configuration
    pub async fn set_plugin_config(&self, plugin_name: &str, config: &str) -> Result<()> {
        debug!("Setting plugin config: {}", plugin_name);

        match self {
            Database::Sqlite(pool) => {
                sqlx::query("INSERT OR REPLACE INTO plugin_configs (plugin_name, config, updated_at) VALUES (?, ?, datetime('now'))")
                    .bind(plugin_name)
                    .bind(config)
                    .execute(pool)
                    .await?;
            }
            Database::Postgres(pool) => {
                sqlx::query("INSERT INTO plugin_configs (plugin_name, config, updated_at) VALUES ($1, $2, NOW()) ON CONFLICT (plugin_name) DO UPDATE SET config = $2, updated_at = NOW()")
                    .bind(plugin_name)
                    .bind(config)
                    .execute(pool)
                    .await?;
            }
        }

        Ok(())
    }

    /// Delete plugin configuration
    pub async fn delete_plugin_config(&self, plugin_name: &str) -> Result<bool> {
        debug!("Deleting plugin config: {}", plugin_name);

        let rows_affected = match self {
            Database::Sqlite(pool) => {
                sqlx::query("DELETE FROM plugin_configs WHERE plugin_name = ?")
                    .bind(plugin_name)
                    .execute(pool)
                    .await?
                    .rows_affected()
            }
            Database::Postgres(pool) => {
                sqlx::query("DELETE FROM plugin_configs WHERE plugin_name = $1")
                    .bind(plugin_name)
                    .execute(pool)
                    .await?
                    .rows_affected()
            }
        };

        Ok(rows_affected > 0)
    }

    /// Get a session by session_id and account_id
    pub async fn get_session(&self, session_id: &str, account_id: &str) -> Result<Option<(String, String)>> {
        debug!("Getting session: {} for account: {}", session_id, account_id);

        let result = match self {
            Database::Sqlite(pool) => {
                sqlx::query_as::<_, (String, String)>(
                    "SELECT messages, metadata FROM sessions WHERE session_id = ? AND account_id = ?"
                )
                    .bind(session_id)
                    .bind(account_id)
                    .fetch_optional(pool)
                    .await?
            }
            Database::Postgres(pool) => {
                sqlx::query_as::<_, (String, String)>(
                    "SELECT messages, metadata FROM sessions WHERE session_id = $1 AND account_id = $2"
                )
                    .bind(session_id)
                    .bind(account_id)
                    .fetch_optional(pool)
                    .await?
            }
        };

        Ok(result)
    }

    /// Create or update a session
    pub async fn upsert_session(
        &self,
        session_id: &str,
        account_id: &str,
        messages: &str,
        metadata: &str,
    ) -> Result<()> {
        debug!("Upserting session: {} for account: {}", session_id, account_id);

        match self {
            Database::Sqlite(pool) => {
                sqlx::query(
                    "INSERT OR REPLACE INTO sessions (session_id, account_id, messages, metadata, created_at, updated_at)
                     VALUES (?, ?, ?, ?, COALESCE((SELECT created_at FROM sessions WHERE session_id = ?), datetime('now')), datetime('now'))"
                )
                    .bind(session_id)
                    .bind(account_id)
                    .bind(messages)
                    .bind(metadata)
                    .bind(session_id)
                    .execute(pool)
                    .await?;
            }
            Database::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO sessions (session_id, account_id, messages, metadata, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, NOW(), NOW())
                     ON CONFLICT (session_id) DO UPDATE
                     SET account_id = $2, messages = $3, metadata = $4, updated_at = NOW()"
                )
                    .bind(session_id)
                    .bind(account_id)
                    .bind(messages)
                    .bind(metadata)
                    .execute(pool)
                    .await?;
            }
        }

        Ok(())
    }

    /// Delete a session
    pub async fn delete_session(&self, session_id: &str, account_id: &str) -> Result<bool> {
        debug!("Deleting session: {} for account: {}", session_id, account_id);

        let rows_affected = match self {
            Database::Sqlite(pool) => {
                sqlx::query("DELETE FROM sessions WHERE session_id = ? AND account_id = ?")
                    .bind(session_id)
                    .bind(account_id)
                    .execute(pool)
                    .await?
                    .rows_affected()
            }
            Database::Postgres(pool) => {
                sqlx::query("DELETE FROM sessions WHERE session_id = $1 AND account_id = $2")
                    .bind(session_id)
                    .bind(account_id)
                    .execute(pool)
                    .await?
                    .rows_affected()
            }
        };

        Ok(rows_affected > 0)
    }

    /// List all sessions for an account
    pub async fn list_sessions(&self, account_id: &str) -> Result<Vec<String>> {
        debug!("Listing sessions for account: {}", account_id);

        let sessions = match self {
            Database::Sqlite(pool) => {
                sqlx::query_scalar::<_, String>(
                    "SELECT session_id FROM sessions WHERE account_id = ? ORDER BY updated_at DESC"
                )
                    .bind(account_id)
                    .fetch_all(pool)
                    .await?
            }
            Database::Postgres(pool) => {
                sqlx::query_scalar::<_, String>(
                    "SELECT session_id FROM sessions WHERE account_id = $1 ORDER BY updated_at DESC"
                )
                    .bind(account_id)
                    .fetch_all(pool)
                    .await?
            }
        };

        Ok(sessions)
    }

    /// Create a scheduled job
    pub async fn create_scheduled_job(
        &self,
        account_id: &str,
        session_id: Option<&str>,
        prompt: &str,
        context: Option<&str>,
        run_at: &str,
    ) -> Result<i64> {
        debug!("Creating scheduled job for account: {} at {}", account_id, run_at);

        let job_id = match self {
            Database::Sqlite(pool) => {
                sqlx::query_scalar::<_, i64>(
                    "INSERT INTO scheduled_jobs (account_id, session_id, prompt, context, run_at)
                     VALUES (?, ?, ?, ?, ?)
                     RETURNING id"
                )
                    .bind(account_id)
                    .bind(session_id)
                    .bind(prompt)
                    .bind(context)
                    .bind(run_at)
                    .fetch_one(pool)
                    .await?
            }
            Database::Postgres(pool) => {
                sqlx::query_scalar::<_, i64>(
                    "INSERT INTO scheduled_jobs (account_id, session_id, prompt, context, run_at)
                     VALUES ($1, $2, $3, $4, $5)
                     RETURNING id"
                )
                    .bind(account_id)
                    .bind(session_id)
                    .bind(prompt)
                    .bind(context)
                    .bind(run_at)
                    .fetch_one(pool)
                    .await?
            }
        };

        Ok(job_id)
    }

    /// Claim a pending scheduled job (HA-safe using FOR UPDATE SKIP LOCKED)
    /// Returns the job details if one was claimed
    pub async fn claim_scheduled_job(&self, pod_id: &str) -> Result<Option<ScheduledJob>> {
        let job = match self {
            Database::Sqlite(pool) => {
                // SQLite doesn't support FOR UPDATE SKIP LOCKED, but it's single-instance anyway
                // Use a simple UPDATE...RETURNING pattern
                sqlx::query_as::<_, ScheduledJob>(
                    "UPDATE scheduled_jobs
                     SET status = 'running',
                         locked_at = datetime('now'),
                         locked_by = ?
                     WHERE id = (
                         SELECT id FROM scheduled_jobs
                         WHERE status = 'pending'
                           AND run_at <= datetime('now')
                         ORDER BY run_at
                         LIMIT 1
                     )
                     RETURNING id, account_id, session_id, prompt, context, run_at, status, locked_at, locked_by, created_at, completed_at, result, error"
                )
                    .bind(pod_id)
                    .fetch_optional(pool)
                    .await?
            }
            Database::Postgres(pool) => {
                // PostgreSQL supports FOR UPDATE SKIP LOCKED for true HA safety
                sqlx::query_as::<_, ScheduledJob>(
                    "UPDATE scheduled_jobs
                     SET status = 'running',
                         locked_at = NOW(),
                         locked_by = $1
                     WHERE id = (
                         SELECT id FROM scheduled_jobs
                         WHERE status = 'pending'
                           AND run_at <= NOW()
                         ORDER BY run_at
                         LIMIT 1
                         FOR UPDATE SKIP LOCKED
                     )
                     RETURNING id, account_id, session_id, prompt, context, run_at, status, locked_at, locked_by, created_at, completed_at, result, error"
                )
                    .bind(pod_id)
                    .fetch_optional(pool)
                    .await?
            }
        };

        Ok(job)
    }

    /// Mark a scheduled job as completed
    pub async fn complete_scheduled_job(&self, job_id: i64, result: &str) -> Result<()> {
        debug!("Completing scheduled job: {}", job_id);

        match self {
            Database::Sqlite(pool) => {
                sqlx::query(
                    "UPDATE scheduled_jobs
                     SET status = 'completed',
                         completed_at = datetime('now'),
                         result = ?
                     WHERE id = ?"
                )
                    .bind(result)
                    .bind(job_id)
                    .execute(pool)
                    .await?;
            }
            Database::Postgres(pool) => {
                sqlx::query(
                    "UPDATE scheduled_jobs
                     SET status = 'completed',
                         completed_at = NOW(),
                         result = $1
                     WHERE id = $2"
                )
                    .bind(result)
                    .bind(job_id)
                    .execute(pool)
                    .await?;
            }
        }

        Ok(())
    }

    /// Mark a scheduled job as failed
    pub async fn fail_scheduled_job(&self, job_id: i64, error: &str) -> Result<()> {
        debug!("Failing scheduled job: {}", job_id);

        match self {
            Database::Sqlite(pool) => {
                sqlx::query(
                    "UPDATE scheduled_jobs
                     SET status = 'failed',
                         completed_at = datetime('now'),
                         error = ?
                     WHERE id = ?"
                )
                    .bind(error)
                    .bind(job_id)
                    .execute(pool)
                    .await?;
            }
            Database::Postgres(pool) => {
                sqlx::query(
                    "UPDATE scheduled_jobs
                     SET status = 'failed',
                         completed_at = NOW(),
                         error = $1
                     WHERE id = $2"
                )
                    .bind(error)
                    .bind(job_id)
                    .execute(pool)
                    .await?;
            }
        }

        Ok(())
    }

    // ============================================================================
    // Document Store Methods
    // ============================================================================

    /// Set a document in the store (create or update)
    pub async fn set_document(
        &self,
        namespace: &str,
        key: &str,
        value: &str,
        account_id: &str,
        metadata: Option<&str>,
    ) -> Result<()> {
        let metadata = metadata.unwrap_or("{}");
        debug!("Setting document: {}:{} for account: {}", namespace, key, account_id);

        match self {
            Database::Sqlite(pool) => {
                sqlx::query(
                    "INSERT INTO documents (namespace, key, value, account_id, metadata, created_at, updated_at)
                     VALUES (?, ?, ?, ?, ?, datetime('now'), datetime('now'))
                     ON CONFLICT(namespace, key, account_id) DO UPDATE
                     SET value = ?, metadata = ?, updated_at = datetime('now')"
                )
                    .bind(namespace)
                    .bind(key)
                    .bind(value)
                    .bind(account_id)
                    .bind(metadata)
                    .bind(value)
                    .bind(metadata)
                    .execute(pool)
                    .await?;
            }
            Database::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO documents (namespace, key, value, account_id, metadata, created_at, updated_at)
                     VALUES ($1, $2, $3, $4, $5::jsonb, NOW(), NOW())
                     ON CONFLICT(namespace, key, account_id) DO UPDATE
                     SET value = $3, metadata = $5::jsonb, updated_at = NOW()"
                )
                    .bind(namespace)
                    .bind(key)
                    .bind(value)
                    .bind(account_id)
                    .bind(metadata)
                    .execute(pool)
                    .await?;
            }
        }

        Ok(())
    }

    /// Get a document from the store
    pub async fn get_document(
        &self,
        namespace: &str,
        key: &str,
        account_id: &str,
    ) -> Result<Option<Document>> {
        debug!("Getting document: {}:{} for account: {}", namespace, key, account_id);

        match self {
            Database::Sqlite(pool) => {
                let doc = sqlx::query_as::<_, Document>(
                    "SELECT namespace, key, value, account_id, metadata, created_at, updated_at
                     FROM documents
                     WHERE namespace = ? AND key = ? AND account_id = ?"
                )
                    .bind(namespace)
                    .bind(key)
                    .bind(account_id)
                    .fetch_optional(pool)
                    .await?;
                Ok(doc)
            }
            Database::Postgres(pool) => {
                let doc = sqlx::query_as::<_, Document>(
                    "SELECT namespace, key, value, account_id, metadata, created_at, updated_at
                     FROM documents
                     WHERE namespace = $1 AND key = $2 AND account_id = $3"
                )
                    .bind(namespace)
                    .bind(key)
                    .bind(account_id)
                    .fetch_optional(pool)
                    .await?;
                Ok(doc)
            }
        }
    }

    /// Delete a document from the store
    pub async fn delete_document(
        &self,
        namespace: &str,
        key: &str,
        account_id: &str,
    ) -> Result<bool> {
        debug!("Deleting document: {}:{} for account: {}", namespace, key, account_id);

        match self {
            Database::Sqlite(pool) => {
                let result = sqlx::query(
                    "DELETE FROM documents WHERE namespace = ? AND key = ? AND account_id = ?"
                )
                    .bind(namespace)
                    .bind(key)
                    .bind(account_id)
                    .execute(pool)
                    .await?;
                Ok(result.rows_affected() > 0)
            }
            Database::Postgres(pool) => {
                let result = sqlx::query(
                    "DELETE FROM documents WHERE namespace = $1 AND key = $2 AND account_id = $3"
                )
                    .bind(namespace)
                    .bind(key)
                    .bind(account_id)
                    .execute(pool)
                    .await?;
                Ok(result.rows_affected() > 0)
            }
        }
    }

    /// List all document keys in a namespace for an account
    pub async fn list_documents(
        &self,
        namespace: &str,
        account_id: &str,
    ) -> Result<Vec<String>> {
        debug!("Listing documents in namespace: {} for account: {}", namespace, account_id);

        match self {
            Database::Sqlite(pool) => {
                let keys: Vec<(String,)> = sqlx::query_as(
                    "SELECT key FROM documents WHERE namespace = ? AND account_id = ? ORDER BY key"
                )
                    .bind(namespace)
                    .bind(account_id)
                    .fetch_all(pool)
                    .await?;
                Ok(keys.into_iter().map(|(k,)| k).collect())
            }
            Database::Postgres(pool) => {
                let keys: Vec<(String,)> = sqlx::query_as(
                    "SELECT key FROM documents WHERE namespace = $1 AND account_id = $2 ORDER BY key"
                )
                    .bind(namespace)
                    .bind(account_id)
                    .fetch_all(pool)
                    .await?;
                Ok(keys.into_iter().map(|(k,)| k).collect())
            }
        }
    }

    // ========================================
    // LLM Configuration Methods
    // ========================================

    /// List all LLM configurations
    pub async fn list_llm_configs(&self) -> Result<Vec<LlmConfigRecord>> {
        debug!("Listing all LLM configurations");

        match self {
            Database::Sqlite(pool) => {
                let configs = sqlx::query_as::<_, LlmConfigRecord>(
                    "SELECT id, name, host, endpoint, model, context_size, timeout_secs,
                            capabilities, priority, enabled, metadata, account_id, created_at, updated_at
                     FROM llm_configs
                     ORDER BY priority DESC, name"
                )
                    .fetch_all(pool)
                    .await?;
                Ok(configs)
            }
            Database::Postgres(pool) => {
                let configs = sqlx::query_as::<_, LlmConfigRecord>(
                    "SELECT id, name, host, endpoint, model, context_size, timeout_secs,
                            capabilities, priority, enabled, metadata, account_id, created_at, updated_at
                     FROM llm_configs
                     ORDER BY priority DESC, name"
                )
                    .fetch_all(pool)
                    .await?;
                Ok(configs)
            }
        }
    }

    /// Get a specific LLM configuration by ID
    pub async fn get_llm_config(&self, id: &str) -> Result<Option<LlmConfigRecord>> {
        debug!("Getting LLM configuration: {}", id);

        match self {
            Database::Sqlite(pool) => {
                let config = sqlx::query_as::<_, LlmConfigRecord>(
                    "SELECT id, name, host, endpoint, model, context_size, timeout_secs,
                            capabilities, priority, enabled, metadata, account_id, created_at, updated_at
                     FROM llm_configs
                     WHERE id = ?"
                )
                    .bind(id)
                    .fetch_optional(pool)
                    .await?;
                Ok(config)
            }
            Database::Postgres(pool) => {
                let config = sqlx::query_as::<_, LlmConfigRecord>(
                    "SELECT id, name, host, endpoint, model, context_size, timeout_secs,
                            capabilities, priority, enabled, metadata, account_id, created_at, updated_at
                     FROM llm_configs
                     WHERE id = $1"
                )
                    .bind(id)
                    .fetch_optional(pool)
                    .await?;
                Ok(config)
            }
        }
    }

    /// Create or update an LLM configuration
    pub async fn upsert_llm_config(&self, config: &LlmConfigRecord) -> Result<()> {
        debug!("Upserting LLM configuration: {}", config.id);

        match self {
            Database::Sqlite(pool) => {
                sqlx::query(
                    "INSERT INTO llm_configs
                        (id, name, host, endpoint, model, context_size, timeout_secs,
                         capabilities, priority, enabled, metadata, account_id, created_at, updated_at)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'), datetime('now'))
                     ON CONFLICT(id) DO UPDATE SET
                        name = excluded.name,
                        host = excluded.host,
                        endpoint = excluded.endpoint,
                        model = excluded.model,
                        context_size = excluded.context_size,
                        timeout_secs = excluded.timeout_secs,
                        capabilities = excluded.capabilities,
                        priority = excluded.priority,
                        enabled = excluded.enabled,
                        metadata = excluded.metadata,
                        account_id = excluded.account_id,
                        updated_at = datetime('now')"
                )
                    .bind(&config.id)
                    .bind(&config.name)
                    .bind(&config.host)
                    .bind(&config.endpoint)
                    .bind(&config.model)
                    .bind(config.context_size)
                    .bind(config.timeout_secs)
                    .bind(&config.capabilities)
                    .bind(config.priority)
                    .bind(config.enabled)
                    .bind(&config.metadata)
                    .bind(&config.account_id)
                    .execute(pool)
                    .await?;
            }
            Database::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO llm_configs
                        (id, name, host, endpoint, model, context_size, timeout_secs,
                         capabilities, priority, enabled, metadata, account_id)
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                     ON CONFLICT(id) DO UPDATE SET
                        name = EXCLUDED.name,
                        host = EXCLUDED.host,
                        endpoint = EXCLUDED.endpoint,
                        model = EXCLUDED.model,
                        context_size = EXCLUDED.context_size,
                        timeout_secs = EXCLUDED.timeout_secs,
                        capabilities = EXCLUDED.capabilities,
                        priority = EXCLUDED.priority,
                        enabled = EXCLUDED.enabled,
                        metadata = EXCLUDED.metadata,
                        account_id = EXCLUDED.account_id,
                        updated_at = CURRENT_TIMESTAMP"
                )
                    .bind(&config.id)
                    .bind(&config.name)
                    .bind(&config.host)
                    .bind(&config.endpoint)
                    .bind(&config.model)
                    .bind(config.context_size)
                    .bind(config.timeout_secs)
                    .bind(&config.capabilities)
                    .bind(config.priority)
                    .bind(config.enabled)
                    .bind(&config.metadata)
                    .bind(&config.account_id)
                    .execute(pool)
                    .await?;
            }
        }

        Ok(())
    }

    /// Delete an LLM configuration
    pub async fn delete_llm_config(&self, id: &str) -> Result<bool> {
        debug!("Deleting LLM configuration: {}", id);

        let rows_affected = match self {
            Database::Sqlite(pool) => {
                sqlx::query("DELETE FROM llm_configs WHERE id = ?")
                    .bind(id)
                    .execute(pool)
                    .await?
                    .rows_affected()
            }
            Database::Postgres(pool) => {
                sqlx::query("DELETE FROM llm_configs WHERE id = $1")
                    .bind(id)
                    .execute(pool)
                    .await?
                    .rows_affected()
            }
        };

        Ok(rows_affected > 0)
    }

    // ========================================
    // Global Configuration Methods
    // ========================================

    /// Get a global configuration value
    pub async fn get_global_config(&self, key: &str) -> Result<Option<String>> {
        debug!("Getting global config: {}", key);

        match self {
            Database::Sqlite(pool) => {
                let value = sqlx::query_scalar::<_, String>(
                    "SELECT value FROM global_config WHERE key = ?"
                )
                    .bind(key)
                    .fetch_optional(pool)
                    .await?;
                Ok(value)
            }
            Database::Postgres(pool) => {
                let value = sqlx::query_scalar::<_, String>(
                    "SELECT value FROM global_config WHERE key = $1"
                )
                    .bind(key)
                    .fetch_optional(pool)
                    .await?;
                Ok(value)
            }
        }
    }

    /// Set a global configuration value
    pub async fn set_global_config(&self, key: &str, value: &str, description: Option<&str>) -> Result<()> {
        debug!("Setting global config: {} = {}", key, value);

        match self {
            Database::Sqlite(pool) => {
                sqlx::query(
                    "INSERT INTO global_config (key, value, description, updated_at)
                     VALUES (?, ?, ?, datetime('now'))
                     ON CONFLICT(key) DO UPDATE SET
                        value = excluded.value,
                        description = excluded.description,
                        updated_at = datetime('now')"
                )
                    .bind(key)
                    .bind(value)
                    .bind(description)
                    .execute(pool)
                    .await?;
            }
            Database::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO global_config (key, value, description)
                     VALUES ($1, $2, $3)
                     ON CONFLICT(key) DO UPDATE SET
                        value = EXCLUDED.value,
                        description = EXCLUDED.description,
                        updated_at = CURRENT_TIMESTAMP"
                )
                    .bind(key)
                    .bind(value)
                    .bind(description)
                    .execute(pool)
                    .await?;
            }
        }

        Ok(())
    }

    /// Delete a global configuration value
    pub async fn delete_global_config(&self, key: &str) -> Result<bool> {
        debug!("Deleting global config: {}", key);

        let rows_affected = match self {
            Database::Sqlite(pool) => {
                sqlx::query("DELETE FROM global_config WHERE key = ?")
                    .bind(key)
                    .execute(pool)
                    .await?
                    .rows_affected()
            }
            Database::Postgres(pool) => {
                sqlx::query("DELETE FROM global_config WHERE key = $1")
                    .bind(key)
                    .execute(pool)
                    .await?
                    .rows_affected()
            }
        };

        Ok(rows_affected > 0)
    }

    /// Sanitize database URL for logging (hide passwords)
    fn sanitize_url(url: &str) -> String {
        if let Some(at_pos) = url.find('@') {
            if let Some(colon_pos) = url[..at_pos].rfind(':') {
                let mut sanitized = url.to_string();
                sanitized.replace_range(colon_pos + 1..at_pos, "****");
                return sanitized;
            }
        }
        url.to_string()
    }
}

/// Scheduled job record from database
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ScheduledJob {
    pub id: i64,
    pub account_id: String,
    pub session_id: Option<String>,
    pub prompt: String,
    pub context: Option<String>,
    pub run_at: chrono::DateTime<chrono::Utc>,
    pub status: String,
    pub locked_at: Option<chrono::DateTime<chrono::Utc>>,
    pub locked_by: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub result: Option<String>,
    pub error: Option<String>,
}

/// Document record from database
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Document {
    pub namespace: String,
    pub key: String,
    pub value: String,
    pub account_id: String,
    pub metadata: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// LLM configuration record from database
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LlmConfigRecord {
    pub id: String,
    pub name: String,
    pub host: String,
    pub endpoint: String,
    pub model: String,
    pub context_size: i32,
    pub timeout_secs: i32,
    pub capabilities: String,  // JSON array stored as string
    pub priority: i32,
    pub enabled: bool,
    pub metadata: String,      // JSON object stored as string
    pub account_id: Option<String>,  // API key/account ID for the LLM provider
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl LlmConfigRecord {
    /// Parse capabilities from JSON string
    pub fn get_capabilities(&self) -> Result<Vec<String>> {
        serde_json::from_str(&self.capabilities)
            .context("Failed to parse capabilities JSON")
    }

    /// Parse metadata from JSON string
    pub fn get_metadata(&self) -> Result<serde_json::Value> {
        serde_json::from_str(&self.metadata)
            .context("Failed to parse metadata JSON")
    }

    /// Convert to LlmConfig for use in LLM calls
    pub fn to_llm_config(&self) -> crate::llm::LlmConfig {
        crate::llm::LlmConfig {
            host: self.host.clone(),
            endpoint: self.endpoint.clone(),
            timeout_secs: self.timeout_secs as u64,
            account_id: self.account_id.clone(),
        }
    }
}


