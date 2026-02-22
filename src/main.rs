mod llm;
mod tools;
mod plugins;
mod js_runtime;
mod session;
mod proxy;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use llm::{LlmConfig, LlmResponse, OllamaMessage, OllamaOptions, OllamaRequest};
use tools::ToolRegistry;
use plugins::PluginRegistry;
use js_runtime::JsRuntimeConfig;
use session::SessionManager;
use tracing::{info, warn, error, debug, trace};
use tracing_subscriber::{fmt, EnvFilter};

#[derive(Serialize, Deserialize, Default, Debug)]
struct DrakeifyConfig {
    llm_host: String,
    llm_model: String,
    llm_endpoint: String,
    identity: String,
    context_size: u32,
    stream: bool,
    headless: bool,

    // Proxy Mode Configuration
    #[serde(default = "default_proxy_port")]
    proxy_port: u16,

    #[serde(default = "default_proxy_host")]
    proxy_host: String,

    #[serde(default = "default_proxy_session_timeout_mins")]
    proxy_session_timeout_mins: u64,

    // System Prompt
    #[serde(default = "default_system_prompt")]
    system_prompt: String,

    // Logging Configuration
    #[serde(default = "default_log_level")]
    log_level: String,

    #[serde(default = "default_log_to_file")]
    log_to_file: bool,

    #[serde(default = "default_log_file")]
    log_file: String,

    // Session Configuration
    #[serde(default = "default_sessions_dir")]
    sessions_dir: String,

    #[serde(default = "default_auto_save")]
    auto_save: bool,

    #[serde(default)]
    session_id: Option<String>,

    // Tool/Plugin Configuration
    #[serde(default)]
    enabled_tools: Option<Vec<String>>,

    #[serde(default)]
    disabled_tools: Option<Vec<String>>,

    #[serde(default)]
    enabled_plugins: Option<Vec<String>>,

    #[serde(default)]
    disabled_plugins: Option<Vec<String>>,

    // HTTP Configuration
    #[serde(default = "default_allow_http")]
    allow_http: bool,
    #[serde(default = "default_http_timeout_secs")]
    http_timeout_secs: u64,
    #[serde(default = "default_http_max_response_size")]
    http_max_response_size: usize,
    #[serde(default)]
    allowed_domains: Option<Vec<String>>,
}

impl DrakeifyConfig {
    /// Load configuration from file and override with environment variables
    fn load_with_env() -> Result<Self> {
        use std::env;

        // Load base config from file (or use defaults if file doesn't exist)
        let mut config: DrakeifyConfig = confy::load_path("./".to_owned() + env!("CARGO_PKG_NAME") + ".toml")
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
        Ok(config)
    }
}

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

fn init_logging(config: &DrakeifyConfig) -> Result<()> {
    use std::fs::OpenOptions;
    use std::io;

    // Parse log level
    let log_level = match config.log_level.to_lowercase().as_str() {
        "trace" => "trace",
        "debug" => "debug",
        "info" => "info",
        "warn" => "warn",
        "error" => "error",
        _ => "info",
    };

    // Create filter
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level));

    if config.log_to_file {
        // Log to file
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&config.log_file)?;

        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(move || file.try_clone().unwrap())
            .with_ansi(false)
            .init();
    } else {
        // Log to stdout
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(io::stdout)
            .init();
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let config_result = DrakeifyConfig::load_with_env()?;

    // Initialize logging
    init_logging(&config_result)?;

    info!("Drakeify starting up");
    debug!("Configuration loaded: {:?}", config_result);

    // Create JavaScript runtime configuration from agency config
    let js_config = JsRuntimeConfig {
        allow_http: config_result.allow_http,
        http_timeout_secs: config_result.http_timeout_secs,
        http_max_response_size: config_result.http_max_response_size,
        allowed_domains: config_result.allowed_domains.clone(),
    };

    // Initialize tool registry and auto-discover tools
    let mut tool_registry = ToolRegistry::new(
        js_config.clone(),
        config_result.enabled_tools.clone(),
        config_result.disabled_tools.clone()
    )?;
    tool_registry.load_tools_from_dir("tools")?;

    let registered_tools = tool_registry.list_tools();
    info!("Registered {} tools: {:?}", registered_tools.len(), registered_tools);

    // Initialize plugin registry and auto-discover plugins
    let mut plugin_registry = PluginRegistry::new(
        js_config.clone(),
        config_result.enabled_plugins.clone(),
        config_result.disabled_plugins.clone()
    )?;
    plugin_registry.load_plugins_from_dir("plugins")?;

    let registered_plugins = plugin_registry.get_plugins();
    info!("Registered {} plugins", registered_plugins.len());
    for plugin in registered_plugins {
        info!("  - {} (priority: {}, hooks: {:?})", plugin.name, plugin.priority, plugin.hooks);
    }

    let llm_config = LlmConfig {
        host: config_result.llm_host.clone(),
        endpoint: config_result.llm_endpoint.clone(),
        timeout_secs: 900,
    };

    // Initialize session manager
    let mut session_manager = SessionManager::new(&config_result.sessions_dir, config_result.auto_save)?;

    // Load existing session or create new one
    if let Some(session_id) = &config_result.session_id {
        if !session_id.is_empty() {
            match session_manager.load_session(session_id) {
                Ok(_) => {
                    info!("📂 Loaded session: {}", session_id);
                }
                Err(e) => {
                    warn!("⚠️  Failed to load session {}: {}", session_id, e);
                    info!("Creating new session instead...");
                    let new_id = session_manager.new_session()?;
                    info!("📝 Created new session: {}", new_id);
                }
            }
        } else {
            let new_id = session_manager.new_session()?;
            info!("📝 Created new session: {}", new_id);
        }
    } else {
        let new_id = session_manager.new_session()?;
        info!("📝 Created new session: {}", new_id);
    }

    // System message for all conversations (from config)
    let system_message = OllamaMessage {
        role: "system".to_string(),
        content: config_result.system_prompt.clone(),
        tool_calls: vec![],
    };

    if config_result.headless {
        // Headless mode: Start proxy server
        info!("🌐 Starting in proxy mode");

        proxy::start_proxy_server(
            config_result.proxy_host.clone(),
            config_result.proxy_port,
            config_result.llm_host.clone(),
            config_result.llm_model.clone(),
            config_result.llm_endpoint.clone(),
            config_result.context_size,
            config_result.stream,
            js_config.clone(),
            config_result.enabled_tools.clone(),
            config_result.disabled_tools.clone(),
            config_result.enabled_plugins.clone(),
            config_result.disabled_plugins.clone(),
        ).await?;
    } else {
        // Interactive CLI mode
        info!("🤖 Agency Interactive Mode");
        info!("Type 'exit' or 'quit' to end the conversation\n");

        // Load messages from session, or start with system message if new session
        let mut conversation_messages = session_manager.get_messages();
        if conversation_messages.is_empty() {
            conversation_messages.push(system_message);
            session_manager.update_messages(conversation_messages.clone())?;
        }

        loop {
            // Get user input
            print!("You: ");
            use std::io::Write;
            std::io::stdout().flush()?;

            let mut user_input = String::new();
            std::io::stdin().read_line(&mut user_input)?;
            let user_input = user_input.trim();

            // Check for exit commands
            if user_input.eq_ignore_ascii_case("exit") || user_input.eq_ignore_ascii_case("quit") {
                info!("\n👋 Goodbye!");
                break;
            }

            if user_input.is_empty() {
                continue;
            }

            // Add user message to conversation
            let user_message = OllamaMessage {
                role: "user".to_string(),
                content: user_input.to_string(),
                tool_calls: vec![],
            };
            conversation_messages.push(user_message);

            // Run conversation with tool execution loop
            print!("\nAssistant: ");
            std::io::stdout().flush()?;

            let assistant_response = run_conversation(
                &mut conversation_messages,
                &config_result,
                &llm_config,
                &tool_registry,
                &plugin_registry,
            ).await?;

            // Save entire conversation to session (includes all tool calls and responses)
            session_manager.update_messages(conversation_messages.clone())?;

            // Execute on_conversation_turn plugin hook
            let turn_data = serde_json::json!({
                "user_message": user_input,
                "assistant_message": assistant_response,
            });
            plugin_registry.execute_hook("on_conversation_turn", turn_data)?;
        }
    }

    Ok(())
}

/// Run a conversation turn with tool execution loop
/// Returns the assistant's final response content
async fn run_conversation(
    conversation_messages: &mut Vec<OllamaMessage>,
    config: &DrakeifyConfig,
    llm_config: &LlmConfig,
    tool_registry: &ToolRegistry,
    plugin_registry: &PluginRegistry,
) -> Result<String> {
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
            config.headless,
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
