use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::database::{Database, LlmConfigRecord};
use crate::llm::LlmConfig;

/// Context for LLM selection
#[derive(Debug, Clone, Default)]
pub struct SelectionContext {
    pub account_id: String,
    pub session_id: Option<String>,
    pub required_capabilities: Vec<String>,
    pub preferred_id: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Plugin hook for LLM selection
/// Returns an LLM ID to use, or None to continue with default logic
pub type SelectionHook = Arc<dyn Fn(&SelectionContext) -> Option<String> + Send + Sync>;

/// Manages LLM configurations with caching and plugin hooks
pub struct LlmConfigManager {
    db: Database,
    /// Cached LLM configurations (id -> config)
    cache: Arc<RwLock<HashMap<String, LlmConfigRecord>>>,
    /// Cached default LLM ID
    default_id: Arc<RwLock<Option<String>>>,
    /// Plugin selection hooks (ordered by priority)
    selection_hooks: Arc<RwLock<Vec<(i32, SelectionHook)>>>,
    /// Fallback config from environment variables
    env_fallback: Option<LlmConfig>,
}

impl LlmConfigManager {
    /// Create a new LLM config manager
    pub async fn new(db: Database, env_fallback: Option<LlmConfig>) -> Result<Self> {
        let manager = Self {
            db: db.clone(),
            cache: Arc::new(RwLock::new(HashMap::new())),
            default_id: Arc::new(RwLock::new(None)),
            selection_hooks: Arc::new(RwLock::new(Vec::new())),
            env_fallback: env_fallback.clone(),
        };

        // Initial cache load
        manager.refresh_cache().await?;

        // Auto-migrate: If no configs exist and we have an env fallback, create it
        if let Some(env_config) = env_fallback {
            let configs = db.list_llm_configs().await?;
            if configs.is_empty() {
                info!("🔄 Auto-migrating environment LLM config to database");

                // Create a default config from environment variables
                let default_config = LlmConfigRecord {
                    id: "default".to_string(),
                    name: "Default LLM (from environment)".to_string(),
                    host: env_config.host.clone(),
                    endpoint: env_config.endpoint.clone(),
                    model: "default".to_string(),  // Will be overridden by llm_model in config
                    context_size: 8192,
                    timeout_secs: env_config.timeout_secs as i32,
                    capabilities: serde_json::to_string(&Vec::<String>::new())?,
                    priority: 0,
                    enabled: true,
                    metadata: "{}".to_string(),
                    account_id: env_config.account_id.clone(),
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                };

                db.upsert_llm_config(&default_config).await?;

                // Set as default
                db.set_global_config("default_llm_id", "default", Some("Default LLM configuration ID")).await?;

                info!("✓ Created default LLM config from environment variables");

                // Refresh cache to pick up the new config
                manager.refresh_cache().await?;
            }
        }

        Ok(manager)
    }

    /// Refresh the cache from the database
    pub async fn refresh_cache(&self) -> Result<()> {
        debug!("Refreshing LLM config cache");

        // Load all configs
        let configs = self.db.list_llm_configs().await?;
        
        let mut cache = self.cache.write().await;
        cache.clear();
        
        for config in configs {
            cache.insert(config.id.clone(), config);
        }

        // Load default ID
        let default_id = self.db.get_global_config("default_llm_id").await?;
        *self.default_id.write().await = default_id;

        debug!("Cache refreshed: {} configs loaded", cache.len());

        Ok(())
    }

    /// Register a plugin selection hook with priority
    /// Higher priority hooks run first
    pub async fn register_selection_hook(&self, priority: i32, hook: SelectionHook) {
        let mut hooks = self.selection_hooks.write().await;
        hooks.push((priority, hook));
        // Sort by priority (descending)
        hooks.sort_by(|a, b| b.0.cmp(&a.0));
    }

    /// Main selection method - tries hooks, then capabilities, then default
    /// Returns (LlmConfig, model_name)
    pub async fn select(&self, context: SelectionContext) -> Result<(LlmConfig, String)> {
        debug!("Selecting LLM for context: {:?}", context);

        // Refresh cache to ensure we have the latest configs from the database
        // This is important when configs are updated via the Web UI
        if let Err(e) = self.refresh_cache().await {
            warn!("Failed to refresh LLM config cache: {}", e);
            // Continue with stale cache rather than failing
        }

        // 1. Try plugin hooks first (in priority order)
        let hooks = self.selection_hooks.read().await;
        for (_priority, hook) in hooks.iter() {
            if let Some(id) = hook(&context) {
                debug!("Plugin hook selected LLM: {}", id);
                if let Ok(config) = self.get_config(&id).await {
                    return Ok(config);
                } else {
                    warn!("Plugin hook returned invalid LLM ID: {}", id);
                }
            }
        }
        drop(hooks); // Release lock

        // 2. Try capability-based selection
        if !context.required_capabilities.is_empty() {
            if let Ok(Some(config)) = self.select_by_capability(&context.required_capabilities).await {
                debug!("Selected LLM by capability");
                return Ok(config);
            }
        }

        // 3. Try preferred ID
        if let Some(ref id) = context.preferred_id {
            if let Ok(config) = self.get_config(id).await {
                debug!("Using preferred LLM: {}", id);
                return Ok(config);
            }
        }

        // 4. Use default
        self.select_default().await
    }

    /// Get a specific LLM configuration by ID
    /// Returns (LlmConfig, model_name)
    pub async fn get_config(&self, id: &str) -> Result<(LlmConfig, String)> {
        let cache = self.cache.read().await;

        if let Some(record) = cache.get(id) {
            Ok((record.to_llm_config(), record.model.clone()))
        } else {
            anyhow::bail!("LLM configuration not found: {}", id)
        }
    }

    /// Select LLM by required capabilities
    /// Returns the highest priority enabled LLM that has all required capabilities
    /// Returns (LlmConfig, model_name)
    pub async fn select_by_capability(&self, required_caps: &[String]) -> Result<Option<(LlmConfig, String)>> {
        let cache = self.cache.read().await;

        let mut candidates: Vec<&LlmConfigRecord> = cache.values()
            .filter(|c| c.enabled)
            .filter(|c| {
                if let Ok(caps) = c.get_capabilities() {
                    required_caps.iter().all(|req| caps.contains(req))
                } else {
                    false
                }
            })
            .collect();

        // Sort by priority (descending)
        candidates.sort_by(|a, b| b.priority.cmp(&a.priority));

        if let Some(config) = candidates.first() {
            Ok(Some((config.to_llm_config(), config.model.clone())))
        } else {
            Ok(None)
        }
    }

    /// Select the default LLM
    /// Falls back to highest priority enabled config, then env vars if no default is configured
    /// Returns (LlmConfig, model_name)
    ///
    /// Note: When falling back to env vars, the model name must be provided separately
    /// via the DRAKEIFY_LLM_MODEL environment variable
    pub async fn select_default(&self) -> Result<(LlmConfig, String)> {
        // Try database default
        let default_id_opt = {
            let default_id = self.default_id.read().await;
            default_id.clone()
        };

        if let Some(id) = default_id_opt {
            if let Ok(config) = self.get_config(&id).await {
                debug!("Using default LLM from database: {}", id);
                return Ok(config);
            } else {
                warn!("Default LLM ID is invalid: {}", id);
            }
        }

        // If no default is set or it's invalid, try to use the highest priority enabled config
        debug!("No valid default LLM ID, selecting highest priority enabled config");
        let cache = self.cache.read().await;
        let mut enabled_configs: Vec<&LlmConfigRecord> = cache.values()
            .filter(|c| c.enabled)
            .collect();

        // Sort by priority (descending)
        enabled_configs.sort_by(|a, b| b.priority.cmp(&a.priority));

        if let Some(config) = enabled_configs.first() {
            debug!("Using highest priority LLM: {} (priority: {})", config.id, config.priority);
            return Ok((config.to_llm_config(), config.model.clone()));
        }
        drop(cache);

        // Fall back to env vars
        // NOTE: This returns a placeholder model name "default" because the env_fallback
        // doesn't include the model name. The caller must use the model from DrakeifyConfig.
        if let Some(ref env_config) = self.env_fallback {
            debug!("Falling back to environment variable LLM config");
            return Ok((env_config.clone(), "default".to_string()));
        }

        anyhow::bail!("No LLM configuration available (no default set and no env vars)")
    }

    /// List all LLM configurations
    pub async fn list_configs(&self) -> Result<Vec<LlmConfigRecord>> {
        let cache = self.cache.read().await;
        Ok(cache.values().cloned().collect())
    }

    /// Get the default LLM ID (if set)
    pub async fn get_default_id(&self) -> Option<String> {
        self.default_id.read().await.clone()
    }

    /// Set the default LLM ID
    pub async fn set_default_id(&self, id: Option<String>) -> Result<()> {
        // Update database
        if let Some(ref llm_id) = id {
            self.db.set_global_config("default_llm_id", llm_id, Some("Default LLM configuration ID")).await?;
        } else {
            self.db.delete_global_config("default_llm_id").await?;
        }

        // Update cache
        *self.default_id.write().await = id;

        Ok(())
    }
}

