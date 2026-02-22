use serde::{Deserialize, Serialize};
use serde_json::Value;
use reqwest::Client;
use futures_util::StreamExt;
use tokio_util::codec::{FramedRead, LinesCodec};
use anyhow::Result;
use std::time::Duration;
use colored::Colorize;
use std::collections::BTreeMap;

#[derive(Serialize, Debug, Clone)]
pub struct JsonSchema {
    #[serde(rename = "type")]
    pub schema_type: String, // "object", "string", ...
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<BTreeMap<String, JsonSchema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,
}

#[derive(Serialize, Debug, Clone)]
pub struct OllamaOptions {
    pub num_ctx: u32,
}

#[derive(Serialize, Debug, Clone)]
pub struct OllamaFunctionDefinition {
    pub description: String,
    pub name: String,
    pub parameters: JsonSchema,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub required: Vec<String>,
}

#[derive(Serialize, Debug, Clone)]
pub struct OllamaFunction {
    pub r#type: String,
    pub function: OllamaFunctionDefinition,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OllamaToolCall {
    pub id: Option<String>,
    pub function: OllamaFunctionCall,
}

#[derive(Debug, Deserialize, Clone)]
pub struct OllamaFunctionCall {
    pub index: Option<u32>,
    pub name: String,
    #[serde(deserialize_with = "deserialize_arguments")]
    pub arguments: Value,
}

// Custom deserializer that handles both string and object formats
fn deserialize_arguments<'de, D>(deserializer: D) -> Result<Value, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrValue {
        String(String),
        Value(Value),
    }

    match StringOrValue::deserialize(deserializer)? {
        StringOrValue::String(s) => {
            // Parse the string as JSON
            serde_json::from_str(&s).map_err(serde::de::Error::custom)
        }
        StringOrValue::Value(v) => Ok(v),
    }
}

// Custom serializer for OllamaFunctionCall
impl serde::Serialize for OllamaFunctionCall {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("OllamaFunctionCall", 3)?;
        if let Some(index) = self.index {
            state.serialize_field("index", &index)?;
        }
        state.serialize_field("name", &self.name)?;
        // Serialize arguments as a JSON string
        let args_str = serde_json::to_string(&self.arguments).map_err(serde::ser::Error::custom)?;
        state.serialize_field("arguments", &args_str)?;
        state.end()
    }
}

#[derive(Serialize, Debug, Deserialize, Clone)]
pub struct OllamaMessage {
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub tool_calls: Vec<OllamaToolCall>,
}

#[derive(Serialize, Debug, Clone)]
pub struct OllamaRequest {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    pub stream: bool,
    pub think: bool,
    pub options: OllamaOptions,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub messages: Vec<OllamaMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<OllamaFunction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<String>,
}

#[derive(Deserialize, Debug)]
struct OllamaResponse {
    response: Option<String>,
    message: Option<OllamaMessage>,
    model: String,
    thinking: Option<String>,
    done: bool,
    done_reason: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize, Debug)]
struct OpenAIResponse {
    id: String,
    object: String,
    created: u64,
    model: String,
    choices: Vec<OpenAIChoice>,
}

#[derive(Deserialize, Debug)]
struct OpenAIChoice {
    index: u32,
    #[serde(default)]
    delta: Option<OpenAIDelta>,
    #[serde(default)]
    message: Option<OpenAIMessage>,
    finish_reason: Option<String>,
}

// Non-streaming message
#[derive(Deserialize, Debug)]
struct OpenAIMessage {
    role: String,
    content: String,
    #[serde(default)]
    tool_calls: Vec<OpenAIToolCall>,
}

#[derive(Deserialize, Debug)]
struct OpenAIDelta {
    role: Option<String>,
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<OpenAIToolCall>,
}

#[derive(Deserialize, Debug)]
struct OpenAIToolCall {
    id: Option<String>,
    index: u32,
    #[serde(rename = "type")]
    call_type: String,
    function: OpenAIFunctionCall,
}

#[derive(Deserialize, Debug)]
struct OpenAIFunctionCall {
    name: String,
    arguments: String,  // Note: This is a JSON string, not a Value
}

#[derive(Clone)]
pub struct LlmConfig {
    pub host: String,
    pub endpoint: String,
    pub timeout_secs: u64,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            host: "http://localhost:11434".to_string(),
            endpoint: "/api/chat".to_string(),
            timeout_secs: 900,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub tool_calls: Vec<OllamaToolCall>,
    pub content: String,
}

/// Execute a single LLM request and return any tool calls that were made
pub async fn execute_request(
    request: OllamaRequest,
    config: &LlmConfig,
    headless: bool,
    on_stream_chunk: Option<&dyn Fn(String, String, usize) -> Result<String>>
) -> Result<LlmResponse> {
    let client = Client::builder()
        .timeout(Duration::from_secs(config.timeout_secs))
        .read_timeout(Duration::from_secs(config.timeout_secs))
        .build()?;

    let url = format!("{}{}", config.host, config.endpoint);
    if !headless {
        println!("{}", serde_json::to_string_pretty(&request).unwrap());
    }

    let response = client.post(url).json(&request).send().await?;
    let byte_stream = response.bytes_stream().map(|result| {
        result.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    });

    let stream_reader = tokio_util::io::StreamReader::new(byte_stream);
    let mut lines = FramedRead::new(stream_reader, LinesCodec::new());

    let mut tool_calls: Vec<OllamaToolCall> = Vec::new();
    let mut response_content = String::new();
    let mut chunk_index = 0;

    while let Some(line) = lines.next().await {
        match line {
            Ok(content) => {
                // Strip "data: " prefix if present (OpenAI format)
                let json_content = content.strip_prefix("data: ").unwrap_or(&content);

                // Try OpenAI format first
                if let Ok(res) = serde_json::from_str::<OpenAIResponse>(json_content) {
                    for choice in res.choices {
                        // Handle streaming response (delta)
                        if let Some(delta) = choice.delta {
                            if let Some(mut content) = delta.content {
                                // Call on_stream_chunk hook if provided
                                if let Some(callback) = on_stream_chunk {
                                    match callback(content.clone(), response_content.clone(), chunk_index) {
                                        Ok(modified_chunk) => {
                                            content = modified_chunk;
                                        }
                                        Err(e) => {
                                            eprintln!("Error in on_stream_chunk callback: {}", e);
                                            // Continue with original content
                                        }
                                    }
                                }

                                chunk_index += 1;
                                response_content.push_str(&content);
                                if !headless {
                                    print!("{}", content);
                                }
                            }
                            if !delta.tool_calls.is_empty() {
                                if !headless {
                                    println!("\n[Tool calls detected: {:?}]", delta.tool_calls);
                                }

                                // Convert OpenAI tool calls to Ollama format
                                for openai_call in delta.tool_calls {
                                    let arguments: Value = serde_json::from_str(&openai_call.function.arguments)
                                        .unwrap_or(Value::Object(serde_json::Map::new()));

                                    tool_calls.push(OllamaToolCall {
                                        id: openai_call.id,
                                        function: OllamaFunctionCall {
                                            index: Some(openai_call.index),
                                            name: openai_call.function.name,
                                            arguments,
                                        },
                                    });
                                }
                            }
                        }

                        // Handle non-streaming response (message)
                        if let Some(message) = choice.message {
                            response_content.push_str(&message.content);
                            if !headless {
                                print!("{}", message.content);
                            }
                            if !message.tool_calls.is_empty() {
                                if !headless {
                                    println!("\n[Tool calls detected: {:?}]", message.tool_calls);
                                }

                                // Convert OpenAI tool calls to Ollama format
                                for openai_call in message.tool_calls {
                                    let arguments: Value = serde_json::from_str(&openai_call.function.arguments)
                                        .unwrap_or(Value::Object(serde_json::Map::new()));

                                    tool_calls.push(OllamaToolCall {
                                        id: openai_call.id,
                                        function: OllamaFunctionCall {
                                            index: Some(openai_call.index),
                                            name: openai_call.function.name,
                                            arguments,
                                        },
                                    });
                                }
                            }
                        }

                        if let Some(finish_reason) = choice.finish_reason {
                            if !headless {
                                println!("\n[Finished]\n[Finish Reason: {}]", finish_reason);
                            }
                            break;
                        }
                    }
                }
                // Fall back to Ollama format
                else if let Ok(res) = serde_json::from_str::<OllamaResponse>(json_content) {
                    if let Some(error) = res.error {
                        print!("{}", error.red());
                    }

                    if let Some(thinking) = res.thinking {
                        print!("{}", thinking.blue());
                    }

                    if let Some(message) = res.message {
                        if !headless {
                            print!("{}", message.content);
                        }
                        if !message.tool_calls.is_empty() {
                            if !headless {
                                println!("\n[Tool calls detected: {:?}]", message.tool_calls);
                            }
                            tool_calls.extend(message.tool_calls);
                        }
                    }

                    if !headless {
                        print!("{}", res.response.unwrap_or_default());
                    }

                    if res.done {
                        if !headless {
                            println!("\n[Stream Finished]\n[Done Reason: {}]", res.done_reason.unwrap_or_default());
                        }
                        break;
                    }
                } else {
                    // Ignore unparseable content (likely streaming text fragments)
                    // Only log if it looks like it should be JSON
                    if content.trim().starts_with('{') || content.trim().starts_with('[') {
                        println!("Error parsing response: {}", content);
                    }
                }
            }
            Err(err) => {
                eprintln!("Error reading stream: {:?}", err);
                break;
            }
        }
    }
    Ok(LlmResponse {
        tool_calls,
        content: response_content,
    })
}
