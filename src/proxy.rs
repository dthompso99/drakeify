use axum::{
    extract::State,
    http::StatusCode,
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
    pub system: Option<String>,
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
    pub system: Option<String>,
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

/// OpenAI-compatible chat message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: Option<String>,
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
}

/// Convert Anthropic Messages format to OpenAI Chat Completions format
fn anthropic_to_openai(anthropic_req: AnthropicMessagesRequest) -> ChatCompletionRequest {
    let mut openai_messages = Vec::new();

    // Add system message if present
    if let Some(system) = anthropic_req.system {
        openai_messages.push(ChatMessage {
            role: "system".to_string(),
            content: Some(system),
            tool_calls: None,
            tool_call_id: None,
        });
    }

    // Convert messages
    for msg in anthropic_req.messages {
        let content = match msg.content {
            AnthropicContent::Text(text) => Some(text),
            AnthropicContent::Blocks(blocks) => {
                // Extract text from blocks
                let text: String = blocks.iter()
                    .filter_map(|block| match block {
                        AnthropicContentBlock::Text { text } => Some(text.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                Some(text)
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
        text: choice.message.content.clone().unwrap_or_default(),
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
        .layer(cors)
        .with_state(state);

    let addr = format!("{}:{}", host, port);
    info!("🚀 Starting proxy server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Result of the tool execution loop
enum ToolLoopResult {
    /// Final response with no client tool calls
    FinalResponse(String),
    /// Response with client tool calls that need to be executed by the client
    ClientToolCalls(String, Vec<OllamaToolCall>),
}

/// Convert a serde_json::Value to JsonSchema (similar to ToolRegistry::value_to_schema)
fn value_to_schema(value: &Value) -> crate::llm::JsonSchema {
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

/// Convert OpenAI tool definitions to Ollama format
fn convert_client_tools_to_ollama(client_tools: &[ToolDefinition]) -> Vec<OllamaFunction> {
    use crate::llm::OllamaFunctionDefinition;

    client_tools.iter().map(|tool| {
        let parameters = value_to_schema(&tool.function.parameters);

        OllamaFunction {
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

/// Message types for streaming updates
#[derive(Debug, Clone)]
enum StreamMessage {
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

/// Execute the tool loop with streaming updates
/// This is a synchronous function that runs in a blocking task
fn execute_tool_loop_streaming(
    conversation_messages: &mut Vec<OllamaMessage>,
    state: &ProxyState,
    tool_registry: &crate::tools::ToolRegistry,
    plugin_registry: &crate::plugins::PluginRegistry,
    client_tools: &[ToolDefinition],
    tx: &tokio::sync::mpsc::UnboundedSender<StreamMessage>,
) -> Result<ToolLoopResult> {
    let mut assistant_response = String::new();

    let _ = tx.send(StreamMessage::Thinking("Processing request...".to_string()));

    loop {
        // Combine Agency tools + client tools
        let mut combined_tools = tool_registry.get_llm_tools();
        combined_tools.extend(convert_client_tools_to_ollama(client_tools));

        // Build LLM request with current messages and combined tools
        let mut current_request = OllamaRequest {
            model: state.llm_model.clone(),
            prompt: None,
            stream: false,
            think: false,
            options: OllamaOptions {
                num_ctx: state.context_size,
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

        if let Ok(modified_data) = plugin_registry.execute_hook("pre_request", request_data) {
            // Update request with modified data
            if let Some(messages) = modified_data.get("messages") {
                if let Ok(updated_messages) = serde_json::from_value(messages.clone()) {
                    current_request.messages = updated_messages;
                }
            }
        }

        debug!("Sending request to LLM with {} tools", current_request.tools.len());
        let _ = tx.send(StreamMessage::Thinking("Waiting for LLM response...".to_string()));

        // Execute LLM request (no streaming for now, no plugin hooks yet)
        // Use block_on since we're in a blocking task
        let llm_response = tokio::runtime::Handle::current().block_on(
            crate::llm::execute_request(
                current_request,
                &state.llm_config,
                true, // headless mode
                None, // no stream callback for now
            )
        )?;

        // Extract content and tool calls from response
        let final_content = llm_response.content;
        let final_tool_calls = llm_response.tool_calls;

        debug!("LLM response: {} chars, {} tool calls", final_content.len(), final_tool_calls.len());

        // Store the assistant's response
        assistant_response = final_content.clone();

        // If no tool calls, we're done
        if final_tool_calls.is_empty() {
            debug!("No tool calls, returning final response");

            // Send the final content
            let _ = tx.send(StreamMessage::Content(final_content.clone()));

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
            if tool_registry.has_tool(&tool_call.function.name) {
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
            let _ = tx.send(StreamMessage::Thinking(format!("Executing {} tool(s)...", agency_tool_calls.len())));

            for tool_call in &agency_tool_calls {
                let tool_name = &tool_call.function.name;
                let mut args_value = tool_call.function.arguments.clone();

                // Execute on_tool_call plugin hook
                let tool_call_data = serde_json::json!({
                    "tool_name": tool_name,
                    "arguments": args_value
                });
                if let Ok(modified_tool_data) = plugin_registry.execute_hook("on_tool_call", tool_call_data) {
                    // Update arguments with modified data
                    if let Some(modified_args) = modified_tool_data.get("arguments") {
                        args_value = modified_args.clone();
                    }
                }

                debug!("   🔧 Executing tool: {}", tool_name);
                let _ = tx.send(StreamMessage::ToolCall(
                    tool_name.clone(),
                    serde_json::to_string(&args_value).unwrap_or_default()
                ));

                match tool_registry.execute(tool_name, args_value.clone()) {
                    Ok(mut result) => {
                        // Execute on_tool_result plugin hook
                        let tool_result_data = serde_json::json!({
                            "tool_name": tool_name,
                            "arguments": args_value,
                            "result": result
                        });
                        if let Ok(modified_result_data) = plugin_registry.execute_hook("on_tool_result", tool_result_data) {
                            // Update result with modified data
                            if let Some(modified_result) = modified_result_data.get("result") {
                                result = modified_result.clone();
                            }
                        }

                        debug!("   ✅ Tool result: {}", serde_json::to_string_pretty(&result)?);
                        let _ = tx.send(StreamMessage::ToolResult(
                            tool_name.clone(),
                            serde_json::to_string(&result).unwrap_or_default()
                        ));

                        // Add tool result to conversation
                        conversation_messages.push(OllamaMessage {
                            role: "tool".to_string(),
                            content: serde_json::to_string(&result)?,
                            tool_calls: vec![],
                        });
                    }
                    Err(e) => {
                        error!("   ❌ Error executing tool {}: {}", tool_name, e);
                        let _ = tx.send(StreamMessage::Error(format!("Tool {} failed: {}", tool_name, e)));

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
            let _ = tx.send(StreamMessage::Thinking("Processing tool results...".to_string()));
        }

        // If there are client tools, return them to the client
        if !client_tool_calls.is_empty() {
            debug!("Returning {} client tool call(s) to client", client_tool_calls.len());

            // Send the content first
            let _ = tx.send(StreamMessage::Content(final_content.clone()));

            // Send tool calls
            for tc in &client_tool_calls {
                let args_json = serde_json::to_string(&tc.function.arguments).unwrap_or_default();
                let _ = tx.send(StreamMessage::ToolCall(tc.function.name.clone(), args_json));
            }

            // Send done with tool calls
            let _ = tx.send(StreamMessage::DoneWithToolCalls);

            // Add assistant message with client tool calls to conversation
            conversation_messages.push(OllamaMessage {
                role: "assistant".to_string(),
                content: final_content.clone(),
                tool_calls: client_tool_calls.clone(),
            });

            return Ok(ToolLoopResult::ClientToolCalls(final_content, client_tool_calls));
        }
    }

    // Execute post_response plugin hook
    let response_data = serde_json::json!({
        "content": assistant_response
    });
    if let Ok(modified_response) = plugin_registry.execute_hook("post_response", response_data) {
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
    let _ = plugin_registry.execute_hook("on_conversation_turn", turn_data);

    let _ = tx.send(StreamMessage::Done);
    Ok(ToolLoopResult::FinalResponse(assistant_response))
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
                content: msg.content.clone().unwrap_or_default(),
                tool_calls: vec![],
            }
        }).collect();

        let state_clone = state.as_ref().clone();
        let client_tools = request.tools.clone();
        let tx_clone = tx.clone();

        // Execute in blocking task
        let result = tokio::task::spawn_blocking(move || {
            // Create tool and plugin registries
            let mut tool_registry = match crate::tools::ToolRegistry::new(
                state_clone.js_config.clone(),
                state_clone.enabled_tools.clone(),
                state_clone.disabled_tools.clone()
            ) {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx_clone.send(StreamMessage::Error(format!("Failed to create tool registry: {}", e)));
                    return;
                }
            };

            if let Err(e) = tool_registry.load_tools_from_dir("tools") {
                error!("Failed to load tools: {}", e);
            }

            let mut plugin_registry = match crate::plugins::PluginRegistry::new(
                state_clone.js_config.clone(),
                state_clone.enabled_plugins.clone(),
                state_clone.disabled_plugins.clone()
            ) {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx_clone.send(StreamMessage::Error(format!("Failed to create plugin registry: {}", e)));
                    return;
                }
            };

            if let Err(e) = plugin_registry.load_plugins_from_dir("plugins") {
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
    Json(request): Json<AnthropicMessagesRequest>,
) -> Response {
    info!("📨 Received Anthropic Messages API request for model: {}", request.model);

    // Convert Anthropic format to OpenAI format
    let openai_request = anthropic_to_openai(request);

    // Call the existing chat completions handler logic
    let response = chat_completions_handler(State(state), Json(openai_request)).await;

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
        total_chars += system.len();
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

/// Handler for /v1/chat/completions endpoint
async fn chat_completions_handler(
    State(state): State<Arc<ProxyState>>,
    Json(request): Json<ChatCompletionRequest>,
) -> Response {
    info!("📨 Received chat completion request for model: {}", request.model);
    info!("   Messages: {}", request.messages.len());

    // Log message details
    for (i, msg) in request.messages.iter().enumerate() {
        if msg.role == "tool" || msg.tool_call_id.is_some() {
            info!("     Message {}: role={}, tool_call_id={:?}, content_len={}",
                i + 1,
                msg.role,
                msg.tool_call_id,
                msg.content.as_ref().map(|c| c.len()).unwrap_or(0)
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
        return handle_streaming_request(state, request).await;
    }

    // Convert OpenAI messages to Ollama format
    let mut conversation_messages: Vec<OllamaMessage> = request.messages.iter().map(|msg| {
        OllamaMessage {
            role: msg.role.clone(),
            content: msg.content.clone().unwrap_or_default(),
            tool_calls: vec![], // Will be populated by LLM responses
        }
    }).collect();

    // Clone state and client tools for the blocking task
    let state_clone = state.as_ref().clone();
    let client_tools = request.tools.clone();

    // Execute the tool loop in a blocking task (ToolRegistry and PluginRegistry are not Send)
    let result = tokio::task::spawn_blocking(move || {
        // Create tool registry for this request
        let mut tool_registry = crate::tools::ToolRegistry::new(
            state_clone.js_config.clone(),
            state_clone.enabled_tools.clone(),
            state_clone.disabled_tools.clone()
        )?;

        // Load tools
        if let Err(e) = tool_registry.load_tools_from_dir("tools") {
            error!("Failed to load tools: {}", e);
        }

        // Create plugin registry for this request
        let mut plugin_registry = crate::plugins::PluginRegistry::new(
            state_clone.js_config.clone(),
            state_clone.enabled_plugins.clone(),
            state_clone.disabled_plugins.clone()
        )?;

        // Load plugins
        if let Err(e) = plugin_registry.load_plugins_from_dir("plugins") {
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
                        content: Some(final_content),
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
                        content: Some(content.clone()),
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
                                content: Some(format!("Error: Failed to serialize tool calls: {}", e)),
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
                        content: Some(format!("Error: {}", e)),
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
                        content: Some(format!("Internal error: {}", e)),
                        tool_calls: None,
                        tool_call_id: None,
                    },
                    finish_reason: Some("error".to_string()),
                }],
            })).into_response()
        }
    }
}

