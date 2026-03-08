use axum::{
    extract::{State, Path},
    http::{StatusCode, HeaderMap},
    response::{IntoResponse, Response, sse::{Event, Sse}},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{CorsLayer, Any};
use tracing::{info, debug, error, warn};
use anyhow::Result;
use futures_util::stream::Stream;
use tokio_stream::StreamExt;

use crate::llm::{OllamaMessage, OllamaRequest, OllamaOptions, OllamaFunction, OllamaToolCall, OllamaFunctionCall, LlmConfig};
use crate::js_runtime::JsRuntimeConfig;
use crate::database::Database;
use crate::StreamMessage;

/// Extract account_id from Authorization header
/// Format: "Authorization: Bearer <api_key>"
/// Returns the API key as account_id, or "anonymous" if not present
fn extract_account_id(headers: &HeaderMap) -> String {
    headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "anonymous".to_string())
}

/// Anthropic system prompt can be string or array of content blocks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AnthropicSystem {
    Text(String),
    Blocks(Vec<AnthropicContentBlock>),
}

/// Anthropic Messages API request
#[derive(Debug, Deserialize)]
pub struct AnthropicMessagesRequest {
    pub model: String,
    pub messages: Vec<AnthropicMessage>,
    #[serde(default)]
    pub max_tokens: u32,
    #[serde(default)]
    pub tools: Vec<AnthropicTool>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub system: Option<AnthropicSystem>,
}

/// Anthropic message format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicMessage {
    pub role: String,
    pub content: AnthropicContent,
}

/// Anthropic content can be string or array of content blocks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AnthropicContent {
    Text(String),
    Blocks(Vec<AnthropicContentBlock>),
}

/// Anthropic content block
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AnthropicContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

/// Anthropic tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Anthropic Messages API response
#[derive(Debug, Serialize)]
pub struct AnthropicMessagesResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub response_type: String,
    pub role: String,
    pub content: Vec<AnthropicContentBlock>,
    pub model: String,
    pub stop_reason: Option<String>,
    pub usage: AnthropicUsage,
}

#[derive(Debug, Serialize)]
pub struct AnthropicUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// Token count request
#[derive(Debug, Deserialize)]
pub struct AnthropicCountTokensRequest {
    pub model: String,
    pub messages: Vec<AnthropicMessage>,
    #[serde(default)]
    pub system: Option<AnthropicSystem>,
    #[serde(default)]
    pub tools: Vec<AnthropicTool>,
}

/// Token count response
#[derive(Debug, Serialize)]
pub struct AnthropicCountTokensResponse {
    pub input_tokens: u32,
}

/// Models list response
#[derive(Debug, Serialize)]
pub struct ModelsResponse {
    pub object: String,
    pub data: Vec<ModelInfo>,
}

/// Model information
#[derive(Debug, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub owned_by: String,
}

/// OpenAI-compatible chat completion request
#[derive(Debug, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub tools: Vec<ToolDefinition>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
}

/// OpenAI message content can be string or array of content parts
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

impl MessageContent {
    /// Extract text from content, joining multiple parts if needed
    pub fn to_text(&self) -> String {
        match self {
            MessageContent::Text(text) => text.clone(),
            MessageContent::Parts(parts) => {
                parts.iter()
                    .filter_map(|part| match part {
                        ContentPart::Text { text } => Some(text.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
    }
}

/// OpenAI content part (for multimodal messages)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrl },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// OpenAI-compatible chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<MessageContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// OpenAI-compatible tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDefinition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// OpenAI-compatible tool call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

/// OpenAI-compatible chat completion response
#[derive(Debug, Serialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<Choice>,
}

#[derive(Debug, Serialize)]
pub struct Choice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: Option<String>,
}

/// Shared state for the proxy server
#[derive(Clone)]
pub struct ProxyState {
    pub llm_config: LlmConfig,
    pub llm_model: String,
    pub context_size: u32,
    pub stream: bool,
    pub js_config: JsRuntimeConfig,
    pub enabled_tools: Option<Vec<String>>,
    pub disabled_tools: Option<Vec<String>>,
    pub enabled_plugins: Option<Vec<String>>,
    pub disabled_plugins: Option<Vec<String>>,
    pub database: Arc<Database>,
}

/// Convert Anthropic Messages format to OpenAI Chat Completions format
fn anthropic_to_openai(anthropic_req: AnthropicMessagesRequest) -> ChatCompletionRequest {
    let mut openai_messages = Vec::new();

    // Add system message if present
    if let Some(system) = anthropic_req.system {
        let system_text = match system {
            AnthropicSystem::Text(text) => text,
            AnthropicSystem::Blocks(blocks) => {
                // Extract text from blocks
                blocks.iter()
                    .filter_map(|block| match block {
                        AnthropicContentBlock::Text { text } => Some(text.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        };

        openai_messages.push(ChatMessage {
            role: "system".to_string(),
            content: Some(MessageContent::Text(system_text)),
            tool_calls: None,
            tool_call_id: None,
        });
    }

    // Convert messages
    for msg in anthropic_req.messages {
        let content = match msg.content {
            AnthropicContent::Text(text) => Some(MessageContent::Text(text)),
            AnthropicContent::Blocks(blocks) => {
                // Extract text from blocks
                let text: String = blocks.iter()
                    .filter_map(|block| match block {
                        AnthropicContentBlock::Text { text } => Some(text.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                Some(MessageContent::Text(text))
            }
        };

        openai_messages.push(ChatMessage {
            role: msg.role,
            content,
            tool_calls: None,
            tool_call_id: None,
        });
    }

    // Convert tools
    let tools: Vec<ToolDefinition> = anthropic_req.tools.iter().map(|tool| {
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.input_schema.clone(),
            },
        }
    }).collect();

    ChatCompletionRequest {
        model: anthropic_req.model,
        messages: openai_messages,
        tools,
        stream: anthropic_req.stream,
        temperature: anthropic_req.temperature,
        max_tokens: Some(anthropic_req.max_tokens),
    }
}

/// Convert OpenAI response to Anthropic Messages format
fn openai_to_anthropic(openai_resp: ChatCompletionResponse) -> AnthropicMessagesResponse {
    let choice = &openai_resp.choices[0];
    let content = vec![AnthropicContentBlock::Text {
        text: choice.message.content.as_ref().map(|c| c.to_text()).unwrap_or_default(),
    }];

    AnthropicMessagesResponse {
        id: openai_resp.id,
        response_type: "message".to_string(),
        role: "assistant".to_string(),
        content,
        model: openai_resp.model,
        stop_reason: choice.finish_reason.clone(),
        usage: AnthropicUsage {
            input_tokens: 0,
            output_tokens: 0,
        },
    }
}

/// Start the proxy server
pub async fn start_proxy_server(
    host: String,
    port: u16,
    llm_host: String,
    llm_model: String,
    llm_endpoint: String,
    context_size: u32,
    stream: bool,
    js_config: JsRuntimeConfig,
    enabled_tools: Option<Vec<String>>,
    disabled_tools: Option<Vec<String>>,
    enabled_plugins: Option<Vec<String>>,
    disabled_plugins: Option<Vec<String>>,
    database: Database,
) -> Result<()> {
    let llm_config = LlmConfig {
        host: llm_host,
        endpoint: llm_endpoint,
        timeout_secs: 900,
    };

    let state = Arc::new(ProxyState {
        llm_config,
        llm_model,
        context_size,
        stream,
        js_config,
        enabled_tools,
        disabled_tools,
        enabled_plugins,
        disabled_plugins,
        database: Arc::new(database),
    });

    // Configure CORS to allow all origins (for development/testing)
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/v1/chat/completions", post(chat_completions_handler))
        .route("/v1/messages", post(anthropic_messages_handler))
        .route("/v1/messages/count_tokens", post(anthropic_count_tokens_handler))
        .route("/v1/models", get(models_handler))
        .route("/webhook/:plugin_name", post(webhook_handler))
        .layer(cors)
        .with_state(state);

    let addr = format!("{}:{}", host, port);
    info!("🚀 Starting proxy server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Interpolate secrets in a string
/// Replaces ${secret.scope.name} with the actual secret value from the database
async fn interpolate_secrets(text: &str, database: &Database) -> Result<String> {
    let mut result = text.to_string();

    // Find all ${secret.scope.name} patterns
    let re = regex::Regex::new(r"\$\{secret\.([^}]+)\}").unwrap();

    for cap in re.captures_iter(text) {
        let full_match = &cap[0];
        let secret_key = &cap[1];

        // Get the secret value from database
        match database.get_secret(secret_key).await {
            Ok(Some(value)) => {
                result = result.replace(full_match, &value);
            }
            Ok(None) => {
                warn!("Secret not found: {}", secret_key);
                // Leave the placeholder as-is if secret not found
            }
            Err(e) => {
                error!("Failed to get secret {}: {}", secret_key, e);
                // Leave the placeholder as-is on error
            }
        }
    }

    Ok(result)
}

/// Result of the tool execution loop
enum ToolLoopResult {
    /// Final response with no client tool calls
    FinalResponse(String),
    /// Response with client tool calls that need to be executed by the client
    ClientToolCalls(String, Vec<OllamaToolCall>),
}

/// Convert a serde_json::Value to JsonSchema (similar to ToolRegistry::value_to_schema)
pub fn value_to_schema(value: &Value) -> crate::llm::JsonSchema {
    use std::collections::BTreeMap;

    let obj = value.as_object();

    let schema_type = obj
        .and_then(|o| o.get("type"))
        .and_then(|v| v.as_str())
        .unwrap_or("object")
        .to_string();

    let description = obj
        .and_then(|o| o.get("description"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let properties = obj
        .and_then(|o| o.get("properties"))
        .and_then(|v| v.as_object())
        .map(|props| {
            props.iter()
                .map(|(k, v)| (k.clone(), value_to_schema(v)))
                .collect::<BTreeMap<_, _>>()
        });

    let required = obj
        .and_then(|o| o.get("required"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        });

    crate::llm::JsonSchema {
        schema_type,
        description,
        properties,
        required,
    }
}



/// Execute the tool loop with streaming updates
/// This is a thin wrapper around the unified conversation loop
/// This is a synchronous function that runs in a blocking task
fn execute_tool_loop_streaming(
    conversation_messages: &mut Vec<OllamaMessage>,
    state: &ProxyState,
    tool_registry: &crate::tools::ToolRegistry,
    plugin_registry: &crate::plugins::PluginRegistry,
    client_tools: &[ToolDefinition],
    tx: &tokio::sync::mpsc::UnboundedSender<StreamMessage>,
) -> Result<ToolLoopResult> {
    use crate::{ConversationLoopConfig, StreamingMode, execute_unified_conversation_loop};

    // Configure the unified loop for proxy mode
    let loop_config = ConversationLoopConfig {
        llm_config: &state.llm_config,
        llm_model: state.llm_model.clone(),
        context_size: state.context_size,
        tool_registry,
        plugin_registry,
        client_tools: client_tools.to_vec(),
        streaming: StreamingMode::Channel { tx: tx.clone() },
    };

    // Execute the unified loop
    // Use block_on since we're in a blocking task
    let result = tokio::runtime::Handle::current().block_on(
        execute_unified_conversation_loop(conversation_messages.clone(), loop_config)
    )?;

    // Update conversation_messages with the results from the unified loop
    *conversation_messages = result.updated_messages;

    // Convert ConversationLoopResult to ToolLoopResult
    if result.client_tool_calls.is_empty() {
        Ok(ToolLoopResult::FinalResponse(result.content))
    } else {
        Ok(ToolLoopResult::ClientToolCalls(result.content, result.client_tool_calls))
    }
}

/// Execute the tool loop - similar to run_conversation in main.rs but for proxy mode
/// This transparently executes Agency tools behind the scenes
/// This is a synchronous function that runs in a blocking task (non-streaming version)
fn execute_tool_loop_sync(
    conversation_messages: &mut Vec<OllamaMessage>,
    state: &ProxyState,
    tool_registry: &crate::tools::ToolRegistry,
    plugin_registry: &crate::plugins::PluginRegistry,
    client_tools: &[ToolDefinition],
) -> Result<ToolLoopResult> {
    // Create a dummy channel for the streaming version
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    execute_tool_loop_streaming(conversation_messages, state, tool_registry, plugin_registry, client_tools, &tx)
}

/// Handle streaming chat completion request
async fn handle_streaming_request(
    state: Arc<ProxyState>,
    request: ChatCompletionRequest,
    account_id: String,
) -> Response {
    use tokio_stream::wrappers::UnboundedReceiverStream;
    use std::convert::Infallible;

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let model = request.model.clone();

    // Spawn task to handle the streaming
    tokio::spawn(async move {
        // Convert OpenAI messages to Ollama format
        let mut conversation_messages: Vec<OllamaMessage> = request.messages.iter().map(|msg| {
            OllamaMessage {
                role: msg.role.clone(),
                content: msg.content.as_ref().map(|c| c.to_text()).unwrap_or_default(),
                tool_calls: vec![],
            }
        }).collect();

        let state_clone = state.as_ref().clone();
        let client_tools = request.tools.clone();
        let tx_clone = tx.clone();
        let account_id_clone = account_id.clone();

        // Execute in blocking task
        let result = tokio::task::spawn_blocking(move || {
            // Create tool and plugin registries
            let mut tool_registry = match crate::tools::ToolRegistry::new(
                state_clone.js_config.clone(),
                state_clone.enabled_tools.clone(),
                state_clone.disabled_tools.clone(),
                Some(state_clone.database.clone()),
                Some(account_id_clone.clone())
            ) {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx_clone.send(StreamMessage::Error(format!("Failed to create tool registry: {}", e)));
                    return;
                }
            };

            if let Err(e) = tool_registry.load_tools_from_dir("data/tools") {
                error!("Failed to load tools: {}", e);
            }

            let mut plugin_registry = match crate::plugins::PluginRegistry::new(
                state_clone.js_config.clone(),
                state_clone.enabled_plugins.clone(),
                state_clone.disabled_plugins.clone(),
                Some(state_clone.database.clone()),
                Some(account_id_clone.clone()),
                Some(state_clone.llm_config.clone()),
                Some(state_clone.llm_model.clone()),
            ) {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx_clone.send(StreamMessage::Error(format!("Failed to create plugin registry: {}", e)));
                    return;
                }
            };

            if let Err(e) = plugin_registry.load_plugins_from_dir("data/plugins") {
                error!("Failed to load plugins: {}", e);
            }

            // Execute the streaming tool loop
            match execute_tool_loop_streaming(
                &mut conversation_messages,
                &state_clone,
                &tool_registry,
                &plugin_registry,
                &client_tools,
                &tx_clone,
            ) {
                Ok(_) => {
                    // Stream messages are already sent via tx_clone
                }
                Err(e) => {
                    let _ = tx_clone.send(StreamMessage::Error(format!("Error: {}", e)));
                }
            }
        }).await;

        if let Err(e) = result {
            let _ = tx.send(StreamMessage::Error(format!("Task error: {}", e)));
        }
    });

    // Create SSE stream
    let stream = UnboundedReceiverStream::new(rx).map(move |msg| {
        match msg {
            StreamMessage::Content(content) => {
                // Send content chunk
                let chunk = serde_json::json!({
                    "id": format!("chatcmpl-{}", chrono::Utc::now().timestamp()),
                    "object": "chat.completion.chunk",
                    "created": chrono::Utc::now().timestamp(),
                    "model": model.clone(),
                    "choices": [{
                        "index": 0,
                        "delta": {"content": content},
                        "finish_reason": null
                    }]
                });
                Ok::<_, Infallible>(Event::default().data(serde_json::to_string(&chunk).unwrap()))
            }
            StreamMessage::Thinking(status) => {
                // Send thinking/status update
                let chunk = serde_json::json!({
                    "id": format!("chatcmpl-{}", chrono::Utc::now().timestamp()),
                    "object": "chat.completion.chunk",
                    "created": chrono::Utc::now().timestamp(),
                    "model": model.clone(),
                    "choices": [{
                        "index": 0,
                        "delta": {"thinking": status},
                        "finish_reason": null
                    }]
                });
                Ok(Event::default().data(serde_json::to_string(&chunk).unwrap()))
            }
            StreamMessage::ToolCall(tool_name, args) => {
                // Send tool call in OpenAI streaming format
                let tool_call_id = format!("call_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0));
                let chunk = serde_json::json!({
                    "id": format!("chatcmpl-{}", chrono::Utc::now().timestamp()),
                    "object": "chat.completion.chunk",
                    "created": chrono::Utc::now().timestamp(),
                    "model": model.clone(),
                    "choices": [{
                        "index": 0,
                        "delta": {
                            "tool_calls": [{
                                "index": 0,
                                "id": tool_call_id,
                                "type": "function",
                                "function": {
                                    "name": tool_name,
                                    "arguments": args
                                }
                            }]
                        },
                        "finish_reason": null
                    }]
                });
                Ok(Event::default().data(serde_json::to_string(&chunk).unwrap()))
            }
            StreamMessage::ToolResult(tool_name, _result) => {
                // Send tool result notification
                let chunk = serde_json::json!({
                    "id": format!("chatcmpl-{}", chrono::Utc::now().timestamp()),
                    "object": "chat.completion.chunk",
                    "created": chrono::Utc::now().timestamp(),
                    "model": model.clone(),
                    "choices": [{
                        "index": 0,
                        "delta": {"thinking": format!("✅ Tool {} completed", tool_name)},
                        "finish_reason": null
                    }]
                });
                Ok(Event::default().data(serde_json::to_string(&chunk).unwrap()))
            }
            StreamMessage::Error(e) => {
                let error_chunk = serde_json::json!({
                    "id": format!("chatcmpl-{}", chrono::Utc::now().timestamp()),
                    "object": "chat.completion.chunk",
                    "created": chrono::Utc::now().timestamp(),
                    "model": model.clone(),
                    "choices": [{
                        "index": 0,
                        "delta": {"content": format!("\n\nError: {}\n", e)},
                        "finish_reason": "error"
                    }]
                });
                Ok(Event::default().data(serde_json::to_string(&error_chunk).unwrap()))
            }
            StreamMessage::Done => {
                // Send final done chunk
                let chunk = serde_json::json!({
                    "id": format!("chatcmpl-{}", chrono::Utc::now().timestamp()),
                    "object": "chat.completion.chunk",
                    "created": chrono::Utc::now().timestamp(),
                    "model": model.clone(),
                    "choices": [{
                        "index": 0,
                        "delta": {},
                        "finish_reason": "stop"
                    }]
                });
                Ok(Event::default().data(serde_json::to_string(&chunk).unwrap()))
            }
            StreamMessage::DoneWithToolCalls => {
                // Send final done chunk with tool_calls finish reason
                let chunk = serde_json::json!({
                    "id": format!("chatcmpl-{}", chrono::Utc::now().timestamp()),
                    "object": "chat.completion.chunk",
                    "created": chrono::Utc::now().timestamp(),
                    "model": model.clone(),
                    "choices": [{
                        "index": 0,
                        "delta": {},
                        "finish_reason": "tool_calls"
                    }]
                });
                Ok(Event::default().data(serde_json::to_string(&chunk).unwrap()))
            }
        }
    });

    Sse::new(stream).into_response()
}

/// Handler for /v1/messages endpoint (Anthropic Messages API)
async fn anthropic_messages_handler(
    State(state): State<Arc<ProxyState>>,
    headers: HeaderMap,
    Json(request): Json<AnthropicMessagesRequest>,
) -> Response {
    info!("📨 Received Anthropic Messages API request for model: {}", request.model);

    // Convert Anthropic format to OpenAI format
    let openai_request = anthropic_to_openai(request);

    // Call the existing chat completions handler logic
    let response = chat_completions_handler(State(state), headers, Json(openai_request)).await;

    // For now, return the response as-is
    // TODO: Convert OpenAI response back to Anthropic format for non-streaming
    response
}

/// Handler for /v1/messages/count_tokens endpoint
async fn anthropic_count_tokens_handler(
    State(_state): State<Arc<ProxyState>>,
    Json(request): Json<AnthropicCountTokensRequest>,
) -> Response {
    info!("📊 Received token count request for model: {}", request.model);

    // Simple token estimation: ~4 characters per token
    let mut total_chars = 0;

    if let Some(system) = &request.system {
        match system {
            AnthropicSystem::Text(text) => {
                total_chars += text.len();
            }
            AnthropicSystem::Blocks(blocks) => {
                for block in blocks {
                    match block {
                        AnthropicContentBlock::Text { text } => {
                            total_chars += text.len();
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    for msg in &request.messages {
        match &msg.content {
            AnthropicContent::Text(text) => {
                total_chars += text.len();
            }
            AnthropicContent::Blocks(blocks) => {
                for block in blocks {
                    match block {
                        AnthropicContentBlock::Text { text } => {
                            total_chars += text.len();
                        }
                        AnthropicContentBlock::ToolUse { name, input, .. } => {
                            total_chars += name.len();
                            total_chars += input.to_string().len();
                        }
                        AnthropicContentBlock::ToolResult { content, .. } => {
                            total_chars += content.len();
                        }
                    }
                }
            }
        }
    }

    // Add tool definitions
    for tool in &request.tools {
        total_chars += tool.name.len();
        total_chars += tool.description.len();
        total_chars += tool.input_schema.to_string().len();
    }

    let estimated_tokens = (total_chars / 4).max(1) as u32;

    let response = AnthropicCountTokensResponse {
        input_tokens: estimated_tokens,
    };

    info!("📊 Estimated {} input tokens", estimated_tokens);

    (StatusCode::OK, Json(response)).into_response()
}

/// Handler for /v1/models endpoint
async fn models_handler(
    State(state): State<Arc<ProxyState>>,
) -> Response {
    info!("📋 Received models list request");

    let model_info = ModelInfo {
        id: state.llm_model.clone(),
        object: "model".to_string(),
        created: 1677649963, // Static timestamp
        owned_by: "drakeify".to_string(),
    };

    let response = ModelsResponse {
        object: "list".to_string(),
        data: vec![model_info],
    };

    (StatusCode::OK, Json(response)).into_response()
}

/// Handler for /webhook/:plugin_name endpoint
async fn webhook_handler(
    State(state): State<Arc<ProxyState>>,
    Path(plugin_name): Path<String>,
    Json(payload): Json<Value>,
) -> Response {
    info!("🪝 Received webhook call for plugin: {}", plugin_name);

    // Clone state and plugin_name for blocking task
    let state_clone = state.as_ref().clone();
    let plugin_name_clone = plugin_name.clone();

    // Execute webhook in a blocking task (PluginRegistry is not Send)
    let result = tokio::task::spawn_blocking(move || {
        // Create plugin registry with default webhook account_id
        // Plugin can call set_account_id() to change it based on payload
        let webhook_account_id = format!("webhook:{}", plugin_name_clone);
        let mut plugin_registry = crate::plugins::PluginRegistry::new(
            state_clone.js_config.clone(),
            state_clone.enabled_plugins.clone(),
            state_clone.disabled_plugins.clone(),
            Some(state_clone.database.clone()),
            Some(webhook_account_id),
            Some(state_clone.llm_config.clone()),
            Some(state_clone.llm_model.clone()),
        )?;

        // Load plugins
        if let Err(e) = plugin_registry.load_plugins_from_dir("data/plugins") {
            error!("Failed to load plugins: {}", e);
            return Err(anyhow::anyhow!("Failed to load plugins: {}", e));
        }

        // Execute webhook hook for the specific plugin
        let webhook_data = serde_json::json!({
            "payload": payload,
        });

        plugin_registry.execute_webhook_hook(&plugin_name_clone, webhook_data)
    }).await;

    match result {
        Ok(Ok(response_data)) => {
            info!("✅ Webhook executed successfully for plugin: {}", plugin_name);
            (StatusCode::OK, Json(response_data)).into_response()
        }
        Ok(Err(e)) => {
            error!("❌ Webhook execution failed for plugin {}: {}", plugin_name, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": format!("Webhook execution failed: {}", e)
                }))
            ).into_response()
        }
        Err(e) => {
            error!("❌ Webhook task failed for plugin {}: {}", plugin_name, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": format!("Webhook task failed: {}", e)
                }))
            ).into_response()
        }
    }
}

/// Handler for /v1/chat/completions endpoint
async fn chat_completions_handler(
    State(state): State<Arc<ProxyState>>,
    headers: HeaderMap,
    Json(request): Json<ChatCompletionRequest>,
) -> Response {
    // Extract account_id from Authorization header
    let account_id = extract_account_id(&headers);
    info!("📨 Received chat completion request for model: {} (account: {})", request.model, account_id);
    info!("   Messages: {}", request.messages.len());

    // Log message details
    for (i, msg) in request.messages.iter().enumerate() {
        if msg.role == "tool" || msg.tool_call_id.is_some() {
            info!("     Message {}: role={}, tool_call_id={:?}, content_len={}",
                i + 1,
                msg.role,
                msg.tool_call_id,
                msg.content.as_ref().map(|c| c.to_text().len()).unwrap_or(0)
            );
        }
    }

    info!("   Client tools: {}", request.tools.len());

    if !request.tools.is_empty() {
        info!("   📦 Client tools received:");
        for (i, tool) in request.tools.iter().enumerate() {
            info!("     {}. {} - {}", i + 1, tool.function.name, tool.function.description);
            // Log the parameters schema
            if let Some(params_obj) = tool.function.parameters.as_object() {
                if let Some(props) = params_obj.get("properties") {
                    if let Some(props_obj) = props.as_object() {
                        info!("        Parameters: {}", props_obj.keys().cloned().collect::<Vec<_>>().join(", "));
                    }
                }
            }
        }
    } else {
        info!("   ℹ️  No client tools in request");
    }

    debug!("Request details: {:?}", request);

    // Check if streaming is requested
    if request.stream {
        info!("📡 Streaming mode requested");
        return handle_streaming_request(state, request, account_id).await;
    }

    // Convert OpenAI messages to Ollama format
    let mut conversation_messages: Vec<OllamaMessage> = request.messages.iter().map(|msg| {
        OllamaMessage {
            role: msg.role.clone(),
            content: msg.content.as_ref().map(|c| c.to_text()).unwrap_or_default(),
            tool_calls: vec![], // Will be populated by LLM responses
        }
    }).collect();

    // Clone state, client tools, and account_id for the blocking task
    let state_clone = state.as_ref().clone();
    let client_tools = request.tools.clone();
    let account_id_clone = account_id.clone();

    // Execute the tool loop in a blocking task (ToolRegistry and PluginRegistry are not Send)
    let result = tokio::task::spawn_blocking(move || {
        // Create tool registry for this request
        let mut tool_registry = crate::tools::ToolRegistry::new(
            state_clone.js_config.clone(),
            state_clone.enabled_tools.clone(),
            state_clone.disabled_tools.clone(),
            Some(state_clone.database.clone()),
            Some(account_id_clone.clone())
        )?;

        // Load tools
        if let Err(e) = tool_registry.load_tools_from_dir("data/tools") {
            error!("Failed to load tools: {}", e);
        }

        // Create plugin registry for this request
        let mut plugin_registry = crate::plugins::PluginRegistry::new(
            state_clone.js_config.clone(),
            state_clone.enabled_plugins.clone(),
            state_clone.disabled_plugins.clone(),
            Some(state_clone.database.clone()),
            Some(account_id_clone.clone()),
            Some(state_clone.llm_config.clone()),
            Some(state_clone.llm_model.clone()),
        )?;

        // Load plugins
        if let Err(e) = plugin_registry.load_plugins_from_dir("data/plugins") {
            error!("Failed to load plugins: {}", e);
        }

        // Execute the tool loop with plugin support and client tools
        execute_tool_loop_sync(
            &mut conversation_messages,
            &state_clone,
            &tool_registry,
            &plugin_registry,
            &client_tools,
        )
    }).await;

    // Handle the result from the blocking task
    match result {
        Ok(Ok(ToolLoopResult::FinalResponse(final_content))) => {
            let response = ChatCompletionResponse {
                id: format!("chatcmpl-{}", chrono::Utc::now().timestamp()),
                object: "chat.completion".to_string(),
                created: chrono::Utc::now().timestamp() as u64,
                model: request.model.clone(),
                choices: vec![Choice {
                    index: 0,
                    message: ChatMessage {
                        role: "assistant".to_string(),
                        content: Some(MessageContent::Text(final_content)),
                        tool_calls: None,
                        tool_call_id: None,
                    },
                    finish_reason: Some("stop".to_string()),
                }],
            };

            info!("✅ Sending final response");
            (StatusCode::OK, Json(response)).into_response()
        }
        Ok(Ok(ToolLoopResult::ClientToolCalls(content, ollama_tool_calls))) => {
            debug!("Converting {} Ollama tool calls to OpenAI format", ollama_tool_calls.len());

            // Convert Ollama tool calls to OpenAI format
            let openai_tool_calls: Vec<ToolCall> = ollama_tool_calls.iter().map(|tc| {
                debug!("  Tool call: {} with args: {:?}", tc.function.name, tc.function.arguments);
                ToolCall {
                    id: format!("call_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)),
                    tool_type: "function".to_string(),
                    function: FunctionCall {
                        name: tc.function.name.clone(),
                        arguments: serde_json::to_string(&tc.function.arguments).unwrap_or_default(),
                    },
                }
            }).collect();

            debug!("Creating response with {} OpenAI tool calls", openai_tool_calls.len());
            let response = ChatCompletionResponse {
                id: format!("chatcmpl-{}", chrono::Utc::now().timestamp()),
                object: "chat.completion".to_string(),
                created: chrono::Utc::now().timestamp() as u64,
                model: request.model.clone(),
                choices: vec![Choice {
                    index: 0,
                    message: ChatMessage {
                        role: "assistant".to_string(),
                        content: Some(MessageContent::Text(content.clone())),
                        tool_calls: Some(openai_tool_calls.clone()),
                        tool_call_id: None,
                    },
                    finish_reason: Some("tool_calls".to_string()),
                }],
            };

            info!("✅ Sending response with {} client tool call(s)", ollama_tool_calls.len());
            debug!("Response: {:?}", response);

            // Try to serialize to JSON to check for errors
            match serde_json::to_string(&response) {
                Ok(json_str) => {
                    debug!("Successfully serialized response ({} bytes)", json_str.len());
                    (StatusCode::OK, Json(response)).into_response()
                }
                Err(e) => {
                    error!("Failed to serialize response: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(ChatCompletionResponse {
                        id: format!("chatcmpl-{}", chrono::Utc::now().timestamp()),
                        object: "chat.completion".to_string(),
                        created: chrono::Utc::now().timestamp() as u64,
                        model: request.model.clone(),
                        choices: vec![Choice {
                            index: 0,
                            message: ChatMessage {
                                role: "assistant".to_string(),
                                content: Some(MessageContent::Text(format!("Error: Failed to serialize tool calls: {}", e))),
                                tool_calls: None,
                                tool_call_id: None,
                            },
                            finish_reason: Some("error".to_string()),
                        }],
                    })).into_response()
                }
            }
        }
        Ok(Err(e)) => {
            error!("Error in tool loop: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ChatCompletionResponse {
                id: format!("chatcmpl-{}", chrono::Utc::now().timestamp()),
                object: "chat.completion".to_string(),
                created: chrono::Utc::now().timestamp() as u64,
                model: request.model.clone(),
                choices: vec![Choice {
                    index: 0,
                    message: ChatMessage {
                        role: "assistant".to_string(),
                        content: Some(MessageContent::Text(format!("Error: {}", e))),
                        tool_calls: None,
                        tool_call_id: None,
                    },
                    finish_reason: Some("error".to_string()),
                }],
            })).into_response()
        }
        Err(e) => {
            error!("Task join error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ChatCompletionResponse {
                id: format!("chatcmpl-{}", chrono::Utc::now().timestamp()),
                object: "chat.completion".to_string(),
                created: chrono::Utc::now().timestamp() as u64,
                model: request.model.clone(),
                choices: vec![Choice {
                    index: 0,
                    message: ChatMessage {
                        role: "assistant".to_string(),
                        content: Some(MessageContent::Text(format!("Internal error: {}", e))),
                        tool_calls: None,
                        tool_call_id: None,
                    },
                    finish_reason: Some("error".to_string()),
                }],
            })).into_response()
        }
    }
}

