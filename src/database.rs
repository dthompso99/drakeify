use anyhow::{Context, Result};
use sqlx::migrate::MigrateDatabase;
use sqlx::{Pool, Sqlite, Postgres};
use tracing::{info, debug};

/// Database abstraction supporting both SQLite and PostgreSQL
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


