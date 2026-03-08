// Drakeify library - shared code for both binaries

pub mod llm;
pub mod tools;
pub mod plugins;
pub mod js_runtime;
pub mod session;
pub mod proxy;
pub mod registry;
pub mod database;
pub mod scheduler;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing_subscriber::EnvFilter;

// Re-export commonly used types
pub use llm::{LlmConfig, OllamaMessage, OllamaOptions, OllamaRequest};
pub use tools::ToolRegistry;
pub use plugins::PluginRegistry;
pub use js_runtime::JsRuntimeConfig;
pub use session::SessionManager;
pub use registry::{RegistryClient, PackageMetadata, PackageType};
pub use database::Database;
pub use scheduler::{SchedulerConfig, start_scheduler};

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct DrakeifyConfig {
    pub llm_host: String,
    pub llm_model: String,
    pub llm_endpoint: String,
    pub identity: String,
    pub context_size: u32,
    pub stream: bool,
    pub headless: bool,

    // Proxy Mode Configuration
    #[serde(default = "default_proxy_port")]
    pub proxy_port: u16,

    #[serde(default = "default_proxy_host")]
    pub proxy_host: String,

    #[serde(default = "default_proxy_session_timeout_mins")]
    pub proxy_session_timeout_mins: u64,

    // System Prompt
    #[serde(default = "default_system_prompt")]
    pub system_prompt: String,

    // Logging Configuration
    #[serde(default = "default_log_level")]
    pub log_level: String,

    #[serde(default = "default_log_to_file")]
    pub log_to_file: bool,

    #[serde(default = "default_log_file")]
    pub log_file: String,

    // Session Configuration
    #[serde(default = "default_sessions_dir")]
    pub sessions_dir: String,

    #[serde(default = "default_auto_save")]
    pub auto_save: bool,

    #[serde(default)]
    pub session_id: Option<String>,

    // Tool/Plugin Configuration
    #[serde(default)]
    pub enabled_tools: Option<Vec<String>>,

    #[serde(default)]
    pub disabled_tools: Option<Vec<String>>,

    #[serde(default)]
    pub enabled_plugins: Option<Vec<String>>,

    #[serde(default)]
    pub disabled_plugins: Option<Vec<String>>,

    // HTTP Configuration
    #[serde(default = "default_allow_http")]
    pub allow_http: bool,
    #[serde(default = "default_http_timeout_secs")]
    pub http_timeout_secs: u64,
    #[serde(default = "default_http_max_response_size")]
    pub http_max_response_size: usize,
    #[serde(default)]
    pub allowed_domains: Option<Vec<String>>,

    // Plugin/Tool Registry Configuration
    #[serde(default = "default_registry_url")]
    pub registry_url: String,
    #[serde(default)]
    pub registry_username: Option<String>,
    #[serde(default)]
    pub registry_password: Option<String>,
    #[serde(default = "default_registry_insecure")]
    pub registry_insecure: bool,

    // Database Configuration
    #[serde(default = "default_database_url")]
    pub database_url: String,

    // Scheduler Configuration
    #[serde(default = "default_scheduler_enabled")]
    pub scheduler_enabled: bool,
    #[serde(default = "default_scheduler_poll_interval_secs")]
    pub scheduler_poll_interval_secs: u64,
    #[serde(default = "default_scheduler_pod_id")]
    pub scheduler_pod_id: String,
}

impl DrakeifyConfig {
    /// Load configuration from file and override with environment variables
    pub fn load_with_env() -> Result<Self> {
        use std::env;

        // Load base config from file (or use defaults if file doesn't exist)
        let mut config: DrakeifyConfig = confy::load_path("./drakeify.toml")
            .unwrap_or_default();

        // Override with environment variables if present
        if let Ok(val) = env::var("DRAKEIFY_LLM_HOST") {
            config.llm_host = val;
        }
        if let Ok(val) = env::var("DRAKEIFY_LLM_MODEL") {
            config.llm_model = val;
        }
        if let Ok(val) = env::var("DRAKEIFY_LLM_ENDPOINT") {
            config.llm_endpoint = val;
        }
        if let Ok(val) = env::var("DRAKEIFY_IDENTITY") {
            config.identity = val;
        }
        if let Ok(val) = env::var("DRAKEIFY_CONTEXT_SIZE") {
            config.context_size = val.parse().unwrap_or(config.context_size);
        }
        if let Ok(val) = env::var("DRAKEIFY_STREAM") {
            config.stream = val.parse().unwrap_or(config.stream);
        }
        if let Ok(val) = env::var("DRAKEIFY_HEADLESS") {
            config.headless = val.parse().unwrap_or(config.headless);
        }
        if let Ok(val) = env::var("DRAKEIFY_PROXY_PORT") {
            config.proxy_port = val.parse().unwrap_or(config.proxy_port);
        }
        if let Ok(val) = env::var("DRAKEIFY_PROXY_HOST") {
            config.proxy_host = val;
        }
        if let Ok(val) = env::var("DRAKEIFY_SYSTEM_PROMPT") {
            config.system_prompt = val;
        }
        if let Ok(val) = env::var("DRAKEIFY_LOG_LEVEL") {
            config.log_level = val;
        }
        if let Ok(val) = env::var("DRAKEIFY_LOG_TO_FILE") {
            config.log_to_file = val.parse().unwrap_or(config.log_to_file);
        }
        if let Ok(val) = env::var("DRAKEIFY_LOG_FILE") {
            config.log_file = val;
        }
        if let Ok(val) = env::var("DRAKEIFY_SESSIONS_DIR") {
            config.sessions_dir = val;
        }
        if let Ok(val) = env::var("DRAKEIFY_AUTO_SAVE") {
            config.auto_save = val.parse().unwrap_or(config.auto_save);
        }
        if let Ok(val) = env::var("DRAKEIFY_ALLOW_HTTP") {
            config.allow_http = val.parse().unwrap_or(config.allow_http);
        }
        if let Ok(val) = env::var("DRAKEIFY_HTTP_TIMEOUT_SECS") {
            config.http_timeout_secs = val.parse().unwrap_or(config.http_timeout_secs);
        }
        if let Ok(val) = env::var("DRAKEIFY_HTTP_MAX_RESPONSE_SIZE") {
            config.http_max_response_size = val.parse().unwrap_or(config.http_max_response_size);
        }
        if let Ok(val) = env::var("DRAKEIFY_REGISTRY_URL") {
            config.registry_url = val;
        }
        if let Ok(val) = env::var("DRAKEIFY_REGISTRY_USERNAME") {
            config.registry_username = Some(val);
        }
        if let Ok(val) = env::var("DRAKEIFY_REGISTRY_PASSWORD") {
            config.registry_password = Some(val);
        }
        if let Ok(val) = env::var("DRAKEIFY_REGISTRY_INSECURE") {
            config.registry_insecure = val.parse().unwrap_or(config.registry_insecure);
        }
        if let Ok(val) = env::var("DRAKEIFY_DATABASE_URL") {
            config.database_url = val;
        }
        if let Ok(val) = env::var("DRAKEIFY_SCHEDULER_ENABLED") {
            config.scheduler_enabled = val.parse().unwrap_or(config.scheduler_enabled);
        }
        if let Ok(val) = env::var("DRAKEIFY_SCHEDULER_POLL_INTERVAL_SECS") {
            config.scheduler_poll_interval_secs = val.parse().unwrap_or(config.scheduler_poll_interval_secs);
        }
        if let Ok(val) = env::var("DRAKEIFY_SCHEDULER_POD_ID") {
            config.scheduler_pod_id = val;
        }

        // Apply defaults for empty string values
        if config.registry_url.is_empty() {
            config.registry_url = default_registry_url();
        }
        if config.database_url.is_empty() {
            config.database_url = default_database_url();
        }
        if config.scheduler_pod_id.is_empty() {
            config.scheduler_pod_id = default_scheduler_pod_id();
        }

        Ok(config)
    }
}

// Default value functions
fn default_system_prompt() -> String {
    "You are a helpful AI assistant with access to tools. Be concise, accurate, and helpful.  Only call a tool if it is the best way to answer the question.".to_string()
}

fn default_proxy_port() -> u16 {
    8080
}

fn default_proxy_host() -> String {
    "0.0.0.0".to_string()
}

fn default_proxy_session_timeout_mins() -> u64 {
    60
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_to_file() -> bool {
    false
}

fn default_log_file() -> String {
    "./agency.log".to_string()
}

fn default_sessions_dir() -> String {
    "./sessions".to_string()
}

fn default_auto_save() -> bool {
    true
}

fn default_allow_http() -> bool {
    true
}

fn default_http_timeout_secs() -> u64 {
    30
}

fn default_http_max_response_size() -> usize {
    10 * 1024 * 1024 // 10MB
}

fn default_registry_url() -> String {
    "https://zot.hallrd.click".to_string()
}

fn default_registry_insecure() -> bool {
    false
}

fn default_database_url() -> String {
    "sqlite://data/drakeify.db".to_string()
}

fn default_scheduler_enabled() -> bool {
    true
}

fn default_scheduler_poll_interval_secs() -> u64 {
    30  // Poll every 30 seconds
}

fn default_scheduler_pod_id() -> String {
    use std::env;
    // Use hostname if available, otherwise generate a random ID
    env::var("HOSTNAME")
        .unwrap_or_else(|_| format!("drakeify-{}", uuid::Uuid::new_v4()))
}

/// Initialize logging based on configuration
pub fn init_logging(config: &DrakeifyConfig) -> Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.log_level));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    Ok(())
}

/// Run a conversation turn with tool execution loop
/// Returns the assistant's final response content
pub async fn run_conversation(
    conversation_messages: &mut Vec<OllamaMessage>,
    config: &DrakeifyConfig,
    llm_config: &LlmConfig,
    tool_registry: &ToolRegistry,
    plugin_registry: &PluginRegistry,
) -> Result<String> {
    use tracing::{debug, error, info};

    let mut assistant_response;

    loop {
        let mut current_request = OllamaRequest {
            model: config.llm_model.clone(),
            prompt: None,
            stream: config.stream,
            think: false,
            options: OllamaOptions {
                num_ctx: config.context_size,
            },
            messages: conversation_messages.clone(),
            tools: tool_registry.get_llm_tools(),
            tool_choice: Some("auto".to_string()),
        };

        // Execute pre_request plugin hook
        let request_data = serde_json::json!({
            "messages": current_request.messages,
            "tools": current_request.tools,
            "options": {
                "num_ctx": current_request.options.num_ctx
            }
        });

        let modified_data = plugin_registry.execute_hook("pre_request", request_data)?;

        // Update request with modified data
        if let Some(messages) = modified_data.get("messages") {
            current_request.messages = serde_json::from_value(messages.clone())?;
        }

        // Create on_stream_chunk callback
        let stream_callback = |chunk: String, accumulated: String, index: usize| -> Result<String> {
            let chunk_data = serde_json::json!({
                "chunk": chunk,
                "accumulated": accumulated,
                "chunk_index": index,
            });
            let modified_data = plugin_registry.execute_hook("on_stream_chunk", chunk_data)?;

            // Extract potentially modified chunk
            let modified_chunk = modified_data.get("chunk")
                .and_then(|v| v.as_str())
                .unwrap_or(&chunk)
                .to_string();

            Ok(modified_chunk)
        };

        // Execute the LLM request
        let llm_response = llm::execute_request(
            current_request,
            &llm_config,
            false, // headless = false for CLI
            Some(&stream_callback)
        ).await?;

        // Execute on_llm_response plugin hook (runs immediately after LLM response, before tool execution)
        let llm_response_data = serde_json::json!({
            "content": llm_response.content,
            "tool_calls": llm_response.tool_calls,
        });
        let modified_llm_response = plugin_registry.execute_hook("on_llm_response", llm_response_data)?;

        // Execute post_response plugin hook (for backwards compatibility)
        let response_data = serde_json::json!({
            "content": modified_llm_response.get("content")
                .and_then(|v| v.as_str())
                .unwrap_or(&llm_response.content),
            "tool_calls": modified_llm_response.get("tool_calls")
                .unwrap_or(&serde_json::json!(llm_response.tool_calls)),
        });
        let modified_response = plugin_registry.execute_hook("post_response", response_data)?;

        // Extract potentially modified content and tool_calls
        let final_content = modified_response.get("content")
            .and_then(|v| v.as_str())
            .unwrap_or(&llm_response.content)
            .to_string();

        let final_tool_calls: Vec<llm::OllamaToolCall> = if let Some(tc) = modified_response.get("tool_calls") {
            serde_json::from_value(tc.clone()).unwrap_or(llm_response.tool_calls.clone())
        } else {
            llm_response.tool_calls.clone()
        };

        // Store the assistant's response
        assistant_response = final_content;

        // If no tool calls, we're done with this turn
        if final_tool_calls.is_empty() {
            break;
        }

        // Execute each tool call
        debug!("Executing {} tool(s)", final_tool_calls.len());

        for tool_call in &final_tool_calls {
            let tool_name = &tool_call.function.name;
            let mut args_value = tool_call.function.arguments.clone();

            // Execute on_tool_call plugin hook
            let tool_call_data = serde_json::json!({
                "tool_name": tool_name,
                "arguments": args_value
            });
            let modified_tool_data = plugin_registry.execute_hook("on_tool_call", tool_call_data)?;

            // Update arguments with modified data
            if let Some(modified_args) = modified_tool_data.get("arguments") {
                args_value = modified_args.clone();
            }

            info!("🔧 Calling tool: {}", tool_name);
            debug!("   Arguments: {}", serde_json::to_string_pretty(&args_value)?);

            match tool_registry.execute(tool_name, args_value.clone()) {
                Ok(mut result) => {
                    // Execute on_tool_result plugin hook
                    let tool_result_data = serde_json::json!({
                        "tool_name": tool_name,
                        "arguments": args_value,
                        "result": result
                    });
                    let modified_result_data = plugin_registry.execute_hook("on_tool_result", tool_result_data)?;

                    // Update result with modified data
                    if let Some(modified_result) = modified_result_data.get("result") {
                        result = modified_result.clone();
                    }

                    debug!("   ✅ Result: {}", serde_json::to_string_pretty(&result)?);

                    // Add tool result to conversation
                    conversation_messages.push(OllamaMessage {
                        role: "tool".to_string(),
                        content: serde_json::to_string(&result)?,
                        tool_calls: vec![],
                    });
                }
                Err(e) => {
                    error!("   ❌ Error executing tool {}: {}", tool_name, e);

                    // Add error to conversation
                    conversation_messages.push(OllamaMessage {
                        role: "tool".to_string(),
                        content: format!("{{\"error\": \"{}\"}}", e),
                        tool_calls: vec![],
                    });
                }
            }
        }

        // Continue loop to send tool results back to LLM
        debug!("Sending tool results back to LLM");
    }

    Ok(assistant_response)
}

/// Execute a conversation loop with tools and plugins
/// This is the core conversation logic used by CLI, proxy, and plugins
/// Returns the final assistant response
pub async fn execute_conversation_loop(
    messages: Vec<OllamaMessage>,
    llm_config: &LlmConfig,
    llm_model: &str,
    context_size: u32,
    tool_registry: &ToolRegistry,
    plugin_registry: &PluginRegistry,
) -> Result<String> {
    // Configure the unified loop for webhook/plugin mode (headless, no client tools)
    let loop_config = ConversationLoopConfig {
        llm_config,
        llm_model: llm_model.to_string(),
        context_size,
        tool_registry,
        plugin_registry,
        client_tools: vec![], // Webhook mode only uses Agency tools
        streaming: StreamingMode::None, // Headless mode
    };

    // Execute the unified loop
    let result = execute_unified_conversation_loop(messages, loop_config).await?;

    // Return just the content (webhook mode doesn't need the updated messages)
    Ok(result.content)
}

// ============================================================================
// Unified Conversation Loop
// ============================================================================

/// Message types for streaming updates
#[derive(Debug, Clone)]
pub enum StreamMessage {
    /// Content chunk from LLM
    Content(String),
    /// Thinking/status update
    Thinking(String),
    /// Tool call being executed
    ToolCall(String, String), // tool_name, args
    /// Tool result
    ToolResult(String, String), // tool_name, result
    /// Error occurred
    Error(String),
    /// Stream finished
    Done,
    /// Stream finished with tool calls
    DoneWithToolCalls,
}

/// Streaming mode configuration
pub enum StreamingMode {
    /// No streaming (headless mode for plugins/webhooks)
    None,

    /// Channel streaming (for proxy)
    Channel {
        tx: tokio::sync::mpsc::UnboundedSender<StreamMessage>,
    },
}

/// Configuration for the unified conversation loop
pub struct ConversationLoopConfig<'a> {
    // LLM configuration
    pub llm_config: &'a LlmConfig,
    pub llm_model: String,
    pub context_size: u32,

    // Registries
    pub tool_registry: &'a ToolRegistry,
    pub plugin_registry: &'a PluginRegistry,

    // Optional client tools (for proxy mode)
    pub client_tools: Vec<crate::proxy::ToolDefinition>,

    // Streaming mode
    pub streaming: StreamingMode,
}

/// Result from conversation loop
pub struct ConversationLoopResult {
    /// Final assistant response content
    pub content: String,

    /// Client tool calls (if any) that need to be executed by the client
    pub client_tool_calls: Vec<llm::OllamaToolCall>,

    /// Updated conversation messages (includes all assistant and tool messages)
    pub updated_messages: Vec<OllamaMessage>,
}

/// Execute a unified conversation loop with tools and plugins
/// This is the single source of truth for all conversation loop logic
///
/// **History-Agnostic**: This function does NOT manage session persistence.
/// The caller is responsible for loading/saving session history as needed.
///
/// # Arguments
/// * `initial_messages` - The conversation history to process
/// * `config` - Configuration for the loop (LLM, tools, streaming, etc.)
///
/// # Returns
/// * `ConversationLoopResult` - Final response and any client tool calls
pub async fn execute_unified_conversation_loop(
    mut conversation_messages: Vec<OllamaMessage>,
    config: ConversationLoopConfig<'_>,
) -> Result<ConversationLoopResult> {
    use tracing::{debug, error};

    let mut assistant_response;

    // Send initial thinking message if streaming
    if let StreamingMode::Channel { ref tx } = config.streaming {
        let _ = tx.send(StreamMessage::Thinking("Processing request...".to_string()));
    }

    loop {
        // Combine Agency tools + client tools
        let mut combined_tools = config.tool_registry.get_llm_tools();

        // Add client tools if provided (proxy mode)
        if !config.client_tools.is_empty() {
            combined_tools.extend(convert_client_tools_to_ollama(&config.client_tools));
        }

        // Build LLM request with current messages and combined tools
        let mut current_request = OllamaRequest {
            model: config.llm_model.clone(),
            prompt: None,
            stream: false,
            think: false,
            options: OllamaOptions {
                num_ctx: config.context_size,
            },
            messages: conversation_messages.clone(),
            tools: combined_tools,
            tool_choice: Some("auto".to_string()),
        };

        // Execute pre_request plugin hook
        let request_data = serde_json::json!({
            "messages": current_request.messages,
            "tools": current_request.tools,
            "options": {
                "num_ctx": current_request.options.num_ctx
            }
        });

        if let Ok(modified_data) = config.plugin_registry.execute_hook("pre_request", request_data) {
            // Update request with modified data
            if let Some(messages) = modified_data.get("messages") {
                if let Ok(updated_messages) = serde_json::from_value(messages.clone()) {
                    current_request.messages = updated_messages;
                }
            }
        }

        debug!("Sending request to LLM with {} tools", current_request.tools.len());

        if let StreamingMode::Channel { ref tx } = config.streaming {
            let _ = tx.send(StreamMessage::Thinking("Waiting for LLM response...".to_string()));
        }

        // Execute LLM request
        let llm_response = llm::execute_request(
            current_request,
            config.llm_config,
            true, // headless mode
            None, // no stream callback
        ).await?;

        // Execute on_llm_response plugin hook
        let llm_response_data = serde_json::json!({
            "content": llm_response.content,
            "tool_calls": llm_response.tool_calls,
        });

        let (final_content, final_tool_calls) = if let Ok(modified) = config.plugin_registry.execute_hook("on_llm_response", llm_response_data) {
            let content = modified.get("content")
                .and_then(|v| v.as_str())
                .unwrap_or(&llm_response.content)
                .to_string();
            let tool_calls = modified.get("tool_calls")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or(llm_response.tool_calls);
            (content, tool_calls)
        } else {
            (llm_response.content, llm_response.tool_calls)
        };

        debug!("LLM response: {} chars, {} tool calls", final_content.len(), final_tool_calls.len());

        // Store the assistant's response
        assistant_response = final_content.clone();

        // If no tool calls, we're done
        if final_tool_calls.is_empty() {
            debug!("No tool calls, returning final response");

            if let StreamingMode::Channel { ref tx } = config.streaming {
                let _ = tx.send(StreamMessage::Content(final_content.clone()));
            }

            // Add final assistant message to conversation (without tool_calls)
            conversation_messages.push(OllamaMessage {
                role: "assistant".to_string(),
                content: final_content,
                tool_calls: vec![],
            });

            break;
        }

        // Separate tool calls into Agency tools and client tools
        let mut agency_tool_calls = vec![];
        let mut client_tool_calls = vec![];

        for tool_call in &final_tool_calls {
            if config.tool_registry.has_tool(&tool_call.function.name) {
                agency_tool_calls.push(tool_call.clone());
            } else {
                client_tool_calls.push(tool_call.clone());
            }
        }

        debug!("Tool calls: {} Agency, {} client", agency_tool_calls.len(), client_tool_calls.len());

        // Execute Agency tools transparently
        if !agency_tool_calls.is_empty() {
            // Add assistant message with Agency tool calls to conversation
            conversation_messages.push(OllamaMessage {
                role: "assistant".to_string(),
                content: final_content.clone(),
                tool_calls: agency_tool_calls.clone(),
            });

            debug!("Executing {} Agency tool(s)", agency_tool_calls.len());

            if let StreamingMode::Channel { ref tx } = config.streaming {
                let _ = tx.send(StreamMessage::Thinking(format!("Executing {} tool(s)...", agency_tool_calls.len())));
            }

            for tool_call in &agency_tool_calls {
                let tool_name = &tool_call.function.name;
                let mut args_value = tool_call.function.arguments.clone();

                // Execute on_tool_call plugin hook
                let tool_call_data = serde_json::json!({
                    "tool_name": tool_name,
                    "arguments": args_value
                });
                if let Ok(modified_tool_data) = config.plugin_registry.execute_hook("on_tool_call", tool_call_data) {
                    // Update arguments with modified data
                    if let Some(modified_args) = modified_tool_data.get("arguments") {
                        args_value = modified_args.clone();
                    }
                }

                debug!("   🔧 Executing tool: {}", tool_name);

                if let StreamingMode::Channel { ref tx } = config.streaming {
                    let _ = tx.send(StreamMessage::ToolCall(
                        tool_name.clone(),
                        serde_json::to_string(&args_value).unwrap_or_default()
                    ));
                }

                match config.tool_registry.execute(tool_name, args_value.clone()) {
                    Ok(mut result) => {
                        // Execute on_tool_result plugin hook
                        let tool_result_data = serde_json::json!({
                            "tool_name": tool_name,
                            "arguments": args_value,
                            "result": result
                        });
                        if let Ok(modified_result_data) = config.plugin_registry.execute_hook("on_tool_result", tool_result_data) {
                            // Update result with modified data
                            if let Some(modified_result) = modified_result_data.get("result") {
                                result = modified_result.clone();
                            }
                        }

                        debug!("   ✅ Tool result: {}", serde_json::to_string_pretty(&result)?);

                        if let StreamingMode::Channel { ref tx } = config.streaming {
                            let _ = tx.send(StreamMessage::ToolResult(
                                tool_name.clone(),
                                serde_json::to_string(&result).unwrap_or_default()
                            ));
                        }

                        // Add tool result to conversation
                        conversation_messages.push(OllamaMessage {
                            role: "tool".to_string(),
                            content: serde_json::to_string(&result)?,
                            tool_calls: vec![],
                        });
                    }
                    Err(e) => {
                        error!("   ❌ Error executing tool {}: {}", tool_name, e);

                        if let StreamingMode::Channel { ref tx } = config.streaming {
                            let _ = tx.send(StreamMessage::Error(format!("Tool {} failed: {}", tool_name, e)));
                        }

                        // Add error to conversation
                        conversation_messages.push(OllamaMessage {
                            role: "tool".to_string(),
                            content: format!("{{\"error\": \"{}\"}}", e),
                            tool_calls: vec![],
                        });
                    }
                }
            }

            // Continue loop to send tool results back to LLM
            debug!("Sending tool results back to LLM");

            if let StreamingMode::Channel { ref tx } = config.streaming {
                let _ = tx.send(StreamMessage::Thinking("Processing tool results...".to_string()));
            }
        }

        // If there are client tools, return them to the client
        if !client_tool_calls.is_empty() {
            debug!("Returning {} client tool call(s) to client", client_tool_calls.len());

            if let StreamingMode::Channel { ref tx } = config.streaming {
                // Send the content first
                let _ = tx.send(StreamMessage::Content(final_content.clone()));

                // Send tool calls
                for tc in &client_tool_calls {
                    let args_json = serde_json::to_string(&tc.function.arguments).unwrap_or_default();
                    let _ = tx.send(StreamMessage::ToolCall(tc.function.name.clone(), args_json));
                }

                // Send done with tool calls
                let _ = tx.send(StreamMessage::DoneWithToolCalls);
            }

            // Add assistant message with client tool calls to conversation
            conversation_messages.push(OllamaMessage {
                role: "assistant".to_string(),
                content: final_content.clone(),
                tool_calls: client_tool_calls.clone(),
            });

            return Ok(ConversationLoopResult {
                content: final_content,
                client_tool_calls,
                updated_messages: conversation_messages,
            });
        }
    }

    // Execute post_response plugin hook
    let response_data = serde_json::json!({
        "content": assistant_response
    });
    if let Ok(modified_response) = config.plugin_registry.execute_hook("post_response", response_data) {
        // Update assistant_response with modified content
        if let Some(modified_content) = modified_response.get("content") {
            if let Some(content_str) = modified_content.as_str() {
                assistant_response = content_str.to_string();
            }
        }
    }

    // Execute on_conversation_turn plugin hook
    let turn_data = serde_json::json!({
        "user_message": conversation_messages.first(),
        "assistant_message": assistant_response.clone(),
        "tool_calls_count": conversation_messages.iter()
            .filter(|m| m.role == "assistant" && !m.tool_calls.is_empty())
            .count()
    });
    let _ = config.plugin_registry.execute_hook("on_conversation_turn", turn_data);

    if let StreamingMode::Channel { ref tx } = config.streaming {
        let _ = tx.send(StreamMessage::Done);
    }

    Ok(ConversationLoopResult {
        content: assistant_response,
        client_tool_calls: vec![],
        updated_messages: conversation_messages,
    })
}

/// Convert OpenAI tool definitions to Ollama format
fn convert_client_tools_to_ollama(client_tools: &[crate::proxy::ToolDefinition]) -> Vec<llm::OllamaFunction> {
    use crate::llm::OllamaFunctionDefinition;
    use crate::proxy::value_to_schema;

    client_tools.iter().map(|tool| {
        let parameters = value_to_schema(&tool.function.parameters);

        llm::OllamaFunction {
            r#type: "function".to_string(),
            function: OllamaFunctionDefinition {
                name: tool.function.name.clone(),
                description: tool.function.description.clone(),
                parameters,
                required: vec![], // Client tools should specify their own required fields in parameters
            },
        }
    }).collect()
}
