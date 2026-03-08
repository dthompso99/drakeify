// Scheduled Task Runner
// Polls the database for pending scheduled tasks and executes them using the unified conversation loop

use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

use crate::database::{Database, ScheduledJob};
use crate::llm::OllamaMessage;
use crate::{ConversationLoopConfig, StreamingMode, execute_unified_conversation_loop};
use crate::{ToolRegistry, PluginRegistry, LlmConfig, JsRuntimeConfig};

/// Configuration for the scheduled task runner
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    pub poll_interval_secs: u64,
    pub pod_id: String,  // Unique identifier for this instance (for HA)
    pub llm_model: String,
    pub llm_config: LlmConfig,
    pub context_size: u32,
    pub js_config: JsRuntimeConfig,
    pub enabled_tools: Option<Vec<String>>,
    pub disabled_tools: Option<Vec<String>>,
    pub enabled_plugins: Option<Vec<String>>,
    pub disabled_plugins: Option<Vec<String>>,
}

/// Start the scheduled task runner
/// This runs in a background task and polls the database for pending jobs
pub async fn start_scheduler(
    database: Database,
    config: SchedulerConfig,
) -> Result<()> {
    info!("🕐 Starting scheduled task runner (pod_id: {})", config.pod_id);
    info!("   Poll interval: {}s", config.poll_interval_secs);

    loop {
        // Try to claim a job
        match database.claim_scheduled_job(&config.pod_id).await {
            Ok(Some(job)) => {
                info!("📋 Claimed scheduled job #{}: {}", job.id, job.prompt);

                // Execute the job in a blocking task (ToolRegistry and PluginRegistry are not Send)
                let db_clone = database.clone();
                let config_clone = config.clone();
                tokio::task::spawn_blocking(move || {
                    // Use block_on to run the async function in the blocking context
                    if let Some(h) = tokio::runtime::Handle::try_current().ok() {
                        if let Err(e) = h.block_on(execute_scheduled_job(db_clone, config_clone, job)) {
                            error!("Failed to execute scheduled job: {}", e);
                        }
                    } else {
                        error!("No tokio runtime available for scheduled job execution");
                    }
                });
            }
            Ok(None) => {
                // No jobs available, sleep and try again
                debug!("No pending scheduled jobs");
            }
            Err(e) => {
                error!("Error claiming scheduled job: {}", e);
            }
        }

        // Sleep before next poll
        sleep(Duration::from_secs(config.poll_interval_secs)).await;
    }
}

/// Execute a single scheduled job
async fn execute_scheduled_job(
    database: Database,
    config: SchedulerConfig,
    job: ScheduledJob,
) -> Result<()> {
    info!("▶️  Executing scheduled job #{}", job.id);
    debug!("   Account: {}", job.account_id);
    debug!("   Session: {:?}", job.session_id);
    debug!("   Prompt: {}", job.prompt);

    // Load session context if session_id is provided
    let mut messages = if let Some(ref session_id) = job.session_id {
        match database.get_session(session_id, &job.account_id).await? {
            Some((messages_json, _metadata_json)) => {
                // Parse messages from JSON
                match serde_json::from_str::<Vec<OllamaMessage>>(&messages_json) {
                    Ok(msgs) => {
                        info!("   Loaded {} messages from session {}", msgs.len(), session_id);
                        msgs
                    }
                    Err(e) => {
                        warn!("Failed to parse session messages: {}, starting fresh", e);
                        vec![]
                    }
                }
            }
            None => {
                warn!("Session {} not found, starting fresh", session_id);
                vec![]
            }
        }
    } else {
        debug!("   No session context, starting fresh");
        vec![]
    };

    // Add the scheduled prompt as a user message
    messages.push(OllamaMessage {
        role: "user".to_string(),
        content: job.prompt.clone(),
        tool_calls: vec![],
    });

    // Wrap database in Arc for registries
    let db_arc = Arc::new(database.clone());

    // Create tool registry for this job
    let mut tool_registry = ToolRegistry::new(
        config.js_config.clone(),
        config.enabled_tools.clone(),
        config.disabled_tools.clone(),
        Some(db_arc.clone()),
        Some(job.account_id.clone()),
    )?;
    tool_registry.set_session_id(job.session_id.clone());

    // Load tools
    if let Err(e) = tool_registry.load_tools_from_dir("data/tools") {
        error!("Failed to load tools: {}", e);
    }

    // Create plugin registry for this job
    let mut plugin_registry = PluginRegistry::new(
        config.js_config.clone(),
        config.enabled_plugins.clone(),
        config.disabled_plugins.clone(),
        Some(db_arc),
        Some(job.account_id.clone()),
        Some(config.llm_config.clone()),
        Some(config.llm_model.clone()),
    )?;

    // Load plugins
    if let Err(e) = plugin_registry.load_plugins_from_dir("data/plugins") {
        error!("Failed to load plugins: {}", e);
    }

    // Execute using the unified conversation loop (headless mode)
    let loop_config = ConversationLoopConfig {
        llm_config: &config.llm_config,
        llm_model: config.llm_model.clone(),
        context_size: config.context_size,
        tool_registry: &tool_registry,
        plugin_registry: &plugin_registry,
        client_tools: vec![],  // Scheduled tasks use Agency tools only
        streaming: StreamingMode::None,  // Headless execution
    };

    match execute_unified_conversation_loop(messages.clone(), loop_config).await {
        Ok(result) => {
            info!("✅ Scheduled job #{} completed successfully", job.id);
            
            // Store result
            let result_json = serde_json::json!({
                "content": result.content,
                "client_tool_calls": result.client_tool_calls,
            }).to_string();

            database.complete_scheduled_job(job.id, &result_json).await?;

            // If there's a session, update it with the new messages
            if let Some(ref session_id) = job.session_id {
                let messages_json = serde_json::to_string(&result.updated_messages)?;

                // Preserve existing metadata
                let metadata_json = match database.get_session(session_id, &job.account_id).await? {
                    Some((_messages, metadata)) => metadata,
                    None => "{}".to_string(),
                };

                database.upsert_session(
                    session_id,
                    &job.account_id,
                    &messages_json,
                    &metadata_json,
                ).await?;

                info!("   Updated session {} with {} messages", session_id, result.updated_messages.len());
            }
        }
        Err(e) => {
            error!("❌ Scheduled job #{} failed: {}", job.id, e);
            database.fail_scheduled_job(job.id, &e.to_string()).await?;
        }
    }

    Ok(())
}

