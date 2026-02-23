// Drakeify library - shared code for both binaries

pub mod llm;
pub mod tools;
pub mod plugins;
pub mod js_runtime;
pub mod session;
pub mod proxy;
pub mod registry;

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

        // Apply defaults for empty string values
        if config.registry_url.is_empty() {
            config.registry_url = default_registry_url();
        }

        Ok(config)
    }
}

// Default value functions
fn default_system_prompt() -> String {
    "You are a helpful AI assistant with access to tools and plugins. Be concise, accurate, and helpful.".to_string()
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

    let mut assistant_response = String::new();

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

        // Execute post_response plugin hook
        let response_data = serde_json::json!({
            "content": llm_response.content,
            "tool_calls": llm_response.tool_calls,
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

