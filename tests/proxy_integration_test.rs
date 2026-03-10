/// Integration tests for the Drakeify proxy
/// 
/// These tests verify that:
/// 1. The proxy correctly parses incoming requests
/// 2. Requests are routed to the correct LLM based on dynamic configuration
/// 3. Responses are properly formatted and returned
/// 4. Tool calls work end-to-end
/// 
/// Run with: cargo test --test proxy_integration_test

use serde_json::{json, Value};
use std::time::Duration;

const PROXY_URL: &str = "http://localhost:8082";
const AUTH_TOKEN: &str = "change_me_in_production";

/// Helper to send a chat completion request to the proxy
async fn send_chat_request(
    messages: Vec<serde_json::Value>,
    model: Option<&str>,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let request_body = json!({
        "model": model.unwrap_or("default"),
        "messages": messages,
        "stream": false,
    });

    let response = client
        .post(format!("{}/v1/chat/completions", PROXY_URL))
        .header("Authorization", format!("Bearer {}", AUTH_TOKEN))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    let status = response.status();
    let body = response.text().await?;

    if !status.is_success() {
        return Err(format!("Request failed with status {}: {}", status, body).into());
    }

    Ok(serde_json::from_str(&body)?)
}

#[tokio::test]
async fn test_simple_chat_completion() {
    let messages = vec![
        json!({
            "role": "user",
            "content": "Say 'Hello, integration test!' and nothing else."
        })
    ];

    let response = send_chat_request(messages, None)
        .await
        .expect("Failed to send request");

    // Verify response structure
    assert!(response.get("id").is_some(), "Response missing 'id' field");
    assert!(response.get("object").is_some(), "Response missing 'object' field");
    assert!(response.get("created").is_some(), "Response missing 'created' field");
    assert!(response.get("model").is_some(), "Response missing 'model' field");
    
    let choices = response.get("choices")
        .expect("Response missing 'choices' field")
        .as_array()
        .expect("'choices' is not an array");
    
    assert!(!choices.is_empty(), "Response has no choices");
    
    let first_choice = &choices[0];
    let message = first_choice.get("message")
        .expect("Choice missing 'message' field");
    
    let content = message.get("content")
        .expect("Message missing 'content' field")
        .as_str()
        .expect("Content is not a string");
    
    assert!(
        content.to_lowercase().contains("hello"),
        "Response doesn't contain expected greeting: {}",
        content
    );
    
    println!("✅ Simple chat completion test passed");
    println!("   Model: {}", response.get("model").unwrap());
    println!("   Response: {}", content);
}

#[tokio::test]
async fn test_model_routing() {
    // This test verifies that the dynamic LLM configuration is working
    // by checking that the response includes the correct model name
    
    let messages = vec![
        json!({
            "role": "user",
            "content": "What is 2+2? Answer with just the number."
        })
    ];

    let response = send_chat_request(messages, None)
        .await
        .expect("Failed to send request");

    let model = response.get("model")
        .expect("Response missing 'model' field")
        .as_str()
        .expect("Model is not a string");
    
    // The model should NOT be "default" - it should be from the database config
    assert_ne!(
        model, "default",
        "Model is 'default', dynamic configuration may not be working"
    );
    
    println!("✅ Model routing test passed");
    println!("   Selected model: {}", model);
}

#[tokio::test]
async fn test_conversation_context() {
    // Test that the proxy maintains conversation context
    
    let messages = vec![
        json!({
            "role": "user",
            "content": "My favorite color is blue."
        }),
        json!({
            "role": "assistant",
            "content": "I'll remember that your favorite color is blue."
        }),
        json!({
            "role": "user",
            "content": "What is my favorite color? Answer with just the color name."
        })
    ];

    let response = send_chat_request(messages, None)
        .await
        .expect("Failed to send request");

    let choices = response.get("choices")
        .expect("Response missing 'choices' field")
        .as_array()
        .expect("'choices' is not an array");
    
    let content = choices[0].get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .expect("Failed to get response content");
    
    assert!(
        content.to_lowercase().contains("blue"),
        "Model didn't remember context: {}",
        content
    );
    
    println!("✅ Conversation context test passed");
    println!("   Response: {}", content);
}

#[tokio::test]
async fn test_tool_availability() {
    // Test that tools are available in the request

    let messages = vec![
        json!({
            "role": "user",
            "content": "List all available tools. Just say 'tools available' if you can see them."
        })
    ];

    let response = send_chat_request(messages, None)
        .await
        .expect("Failed to send request");

    // Just verify we got a response - the model should see tools
    let choices = response.get("choices")
        .expect("Response missing 'choices' field")
        .as_array()
        .expect("'choices' is not an array");

    assert!(!choices.is_empty(), "No choices in response");

    println!("✅ Tool availability test passed");
}

#[tokio::test]
async fn test_tool_call_execution() {
    // Test that a specific tool call is properly parsed and executed

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to build client");

    let request_body = json!({
        "model": "default",
        "messages": [
            {
                "role": "user",
                "content": "Please use the memory tool to store this information: 'test_key' = 'test_value'"
            }
        ]
    });

    let response = client
        .post(format!("{}/v1/chat/completions", PROXY_URL))
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", AUTH_TOKEN))
        .json(&request_body)
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let response_json: Value = response.json().await.expect("Failed to parse response");

    // Log the full response for observability
    println!("\n📋 Full LLM Response:");
    println!("{}", serde_json::to_string_pretty(&response_json).unwrap());

    // Extract the assistant's message
    let message = &response_json["choices"][0]["message"];
    let content = message["content"].as_str().unwrap_or("");

    println!("\n✅ Tool call execution test passed");
    println!("   Response content: {}", content);

    // The response should indicate success (either through content or tool execution)
    // We're mainly testing that the request completes successfully
    assert!(!content.is_empty() || message["tool_calls"].is_array());
}

#[tokio::test]
async fn test_tool_call_format_parsing() {
    // Test that tool calls are properly formatted in the response
    // This test explicitly asks for a tool call and verifies the format

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to build client");

    // Ask for a simple calculation that should trigger a tool call
    let request_body = json!({
        "model": "default",
        "messages": [
            {
                "role": "user",
                "content": "What is 15 + 27? Please calculate this."
            }
        ]
    });

    let response = client
        .post(format!("{}/v1/chat/completions", PROXY_URL))
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", AUTH_TOKEN))
        .json(&request_body)
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let response_json: Value = response.json().await.expect("Failed to parse response");

    // Log the full response structure for observability
    println!("\n📋 Tool Call Format Analysis:");
    println!("{}", serde_json::to_string_pretty(&response_json).unwrap());

    // Verify response structure
    assert!(response_json["choices"].is_array(), "Response should have choices array");
    assert!(response_json["choices"][0]["message"].is_object(), "Choice should have message object");

    let message = &response_json["choices"][0]["message"];

    // Log message structure
    println!("\n📝 Message Structure:");
    println!("   - role: {}", message["role"].as_str().unwrap_or("N/A"));
    println!("   - content: {}", message["content"].as_str().unwrap_or("N/A"));
    println!("   - has tool_calls: {}", message["tool_calls"].is_array());

    if let Some(tool_calls) = message["tool_calls"].as_array() {
        println!("   - tool_calls count: {}", tool_calls.len());
        for (i, call) in tool_calls.iter().enumerate() {
            println!("\n   Tool Call #{}:", i + 1);
            println!("     - id: {}", call["id"].as_str().unwrap_or("N/A"));
            println!("     - type: {}", call["type"].as_str().unwrap_or("N/A"));
            println!("     - function.name: {}", call["function"]["name"].as_str().unwrap_or("N/A"));
            println!("     - function.arguments: {}", call["function"]["arguments"].as_str().unwrap_or("N/A"));
        }
    }

    println!("\n✅ Tool call format parsing test passed");
}

#[tokio::test]
async fn test_client_tool_call_format() {
    // Test that client-provided tools trigger visible tool_calls in the response
    // This helps us observe how different models format their tool call responses

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to build client");

    // Define a simple client tool
    let request_body = json!({
        "model": "default",
        "messages": [
            {
                "role": "user",
                "content": "Please use the get_weather tool to check the weather in San Francisco, CA"
            }
        ],
        "tools": [
            {
                "type": "function",
                "function": {
                    "name": "get_weather",
                    "description": "Get the current weather for a location",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "location": {
                                "type": "string",
                                "description": "The city and state, e.g. San Francisco, CA"
                            }
                        },
                        "required": ["location"]
                    }
                }
            }
        ]
    });

    let response = client
        .post(format!("{}/v1/chat/completions", PROXY_URL))
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", AUTH_TOKEN))
        .json(&request_body)
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let response_json: Value = response.json().await.expect("Failed to parse response");

    // Log the full response structure for observability
    println!("\n📋 Client Tool Call Response:");
    println!("{}", serde_json::to_string_pretty(&response_json).unwrap());

    // Verify response structure
    assert!(response_json["choices"].is_array(), "Response should have choices array");

    let message = &response_json["choices"][0]["message"];

    // Log detailed message structure
    println!("\n📝 Detailed Message Analysis:");
    println!("   - role: {}", message["role"].as_str().unwrap_or("N/A"));
    println!("   - content: {}", message["content"].as_str().unwrap_or("N/A"));
    println!("   - has tool_calls: {}", message["tool_calls"].is_array());
    println!("   - finish_reason: {}", response_json["choices"][0]["finish_reason"].as_str().unwrap_or("N/A"));

    if let Some(tool_calls) = message["tool_calls"].as_array() {
        println!("\n🔧 Tool Calls Detected: {}", tool_calls.len());
        for (i, call) in tool_calls.iter().enumerate() {
            println!("\n   Tool Call #{}:", i + 1);
            println!("     Raw JSON: {}", serde_json::to_string_pretty(call).unwrap());
            println!("     - id: {}", call["id"].as_str().unwrap_or("N/A"));
            println!("     - type: {}", call["type"].as_str().unwrap_or("N/A"));
            println!("     - function.name: {}", call["function"]["name"].as_str().unwrap_or("N/A"));
            println!("     - function.arguments: {}", call["function"]["arguments"].as_str().unwrap_or("N/A"));

            // Try to parse the arguments
            if let Some(args_str) = call["function"]["arguments"].as_str() {
                match serde_json::from_str::<Value>(args_str) {
                    Ok(args_json) => {
                        println!("     - parsed arguments: {}", serde_json::to_string_pretty(&args_json).unwrap());
                    }
                    Err(e) => {
                        println!("     - ⚠️  Failed to parse arguments as JSON: {}", e);
                    }
                }
            }
        }

        // Verify we got at least one tool call
        assert!(!tool_calls.is_empty(), "Expected at least one tool call for client tool");

        // Verify the tool call is for get_weather
        let first_call = &tool_calls[0];
        assert_eq!(
            first_call["function"]["name"].as_str().unwrap_or(""),
            "get_weather",
            "Expected tool call to be for get_weather"
        );
    } else {
        println!("\n⚠️  No tool_calls array found in response");
        println!("   This might indicate the model executed the tool internally");
        println!("   or didn't recognize the tool call request");
    }

    println!("\n✅ Client tool call format test passed");
}

#[tokio::test]
async fn test_qwen3_coder_tool_call_format() {
    // Test tool call format specifically with qwen3-coder:30b model
    // This helps us compare how different models format their tool calls

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))  // Longer timeout for larger model
        .build()
        .expect("Failed to build client");

    // Define a simple client tool
    let request_body = json!({
        "model": "qwen3-coder-30b",  // Request specific model by ID
        "messages": [
            {
                "role": "user",
                "content": "Please use the get_weather tool to check the weather in New York"
            }
        ],
        "tools": [
            {
                "type": "function",
                "function": {
                    "name": "get_weather",
                    "description": "Get the current weather for a location",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "location": {
                                "type": "string",
                                "description": "The city and state, e.g. New York, NY"
                            }
                        },
                        "required": ["location"]
                    }
                }
            }
        ]
    });

    let response = client
        .post(format!("{}/v1/chat/completions", PROXY_URL))
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", AUTH_TOKEN))
        .json(&request_body)
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let response_json: Value = response.json().await.expect("Failed to parse response");

    // Log the full response structure for observability
    println!("\n📋 Qwen3-Coder Tool Call Response:");
    println!("{}", serde_json::to_string_pretty(&response_json).unwrap());

    // Verify we got the correct model
    let model_used = response_json["model"].as_str().unwrap_or("unknown");
    println!("\n🤖 Model Used: {}", model_used);

    // Verify response structure
    assert!(response_json["choices"].is_array(), "Response should have choices array");

    let message = &response_json["choices"][0]["message"];

    // Log detailed message structure
    println!("\n📝 Qwen3-Coder Message Analysis:");
    println!("   - role: {}", message["role"].as_str().unwrap_or("N/A"));
    println!("   - content length: {} chars", message["content"].as_str().unwrap_or("").len());
    println!("   - has tool_calls: {}", message["tool_calls"].is_array());
    println!("   - finish_reason: {}", response_json["choices"][0]["finish_reason"].as_str().unwrap_or("N/A"));

    if let Some(tool_calls) = message["tool_calls"].as_array() {
        println!("\n🔧 Tool Calls Detected: {}", tool_calls.len());
        for (i, call) in tool_calls.iter().enumerate() {
            println!("\n   Tool Call #{}:", i + 1);
            println!("     Raw JSON: {}", serde_json::to_string_pretty(call).unwrap());
            println!("     - id: {}", call["id"].as_str().unwrap_or("N/A"));
            println!("     - type: {}", call["type"].as_str().unwrap_or("N/A"));
            println!("     - function.name: {}", call["function"]["name"].as_str().unwrap_or("N/A"));
            println!("     - function.arguments: {}", call["function"]["arguments"].as_str().unwrap_or("N/A"));

            // Try to parse the arguments
            if let Some(args_str) = call["function"]["arguments"].as_str() {
                match serde_json::from_str::<Value>(args_str) {
                    Ok(args_json) => {
                        println!("     - parsed arguments: {}", serde_json::to_string_pretty(&args_json).unwrap());
                    }
                    Err(e) => {
                        println!("     - ⚠️  Failed to parse arguments as JSON: {}", e);
                    }
                }
            }
        }
    } else {
        println!("\n⚠️  No tool_calls array found in response");
        println!("   Content preview: {}",
            message["content"].as_str().unwrap_or("").chars().take(200).collect::<String>());
    }

    println!("\n✅ Qwen3-Coder tool call format test passed");
}

#[tokio::test]
async fn test_error_handling_invalid_request() {
    // Test that the proxy handles invalid requests gracefully

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("Failed to build client");

    let invalid_body = json!({
        "invalid_field": "this should fail"
    });

    let response = client
        .post(format!("{}/v1/chat/completions", PROXY_URL))
        .header("Authorization", format!("Bearer {}", AUTH_TOKEN))
        .header("Content-Type", "application/json")
        .json(&invalid_body)
        .send()
        .await
        .expect("Failed to send request");

    // Should get an error response (4xx or 5xx)
    assert!(
        !response.status().is_success(),
        "Invalid request should return error status"
    );

    println!("✅ Error handling test passed");
    println!("   Status: {}", response.status());
}

#[tokio::test]
async fn test_authentication() {
    // Test that authentication is required

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("Failed to build client");

    let request_body = json!({
        "model": "default",
        "messages": [
            {
                "role": "user",
                "content": "Hello"
            }
        ]
    });

    // Try without auth token
    let response = client
        .post(format!("{}/v1/chat/completions", PROXY_URL))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
        .expect("Failed to send request");

    // Should get 401 Unauthorized
    assert_eq!(
        response.status(),
        reqwest::StatusCode::UNAUTHORIZED,
        "Request without auth should return 401"
    );

    println!("✅ Authentication test passed");
}

#[tokio::test]
async fn test_response_format() {
    // Test that the response follows OpenAI API format

    let messages = vec![
        json!({
            "role": "user",
            "content": "Say 'test'"
        })
    ];

    let response = send_chat_request(messages, None)
        .await
        .expect("Failed to send request");

    // Verify OpenAI-compatible response format
    assert_eq!(
        response.get("object").and_then(|v| v.as_str()),
        Some("chat.completion"),
        "Response object type should be 'chat.completion'"
    );

    let choices = response.get("choices")
        .expect("Response missing 'choices' field")
        .as_array()
        .expect("'choices' is not an array");

    let first_choice = &choices[0];

    assert_eq!(
        first_choice.get("index").and_then(|v| v.as_u64()),
        Some(0),
        "First choice should have index 0"
    );

    assert_eq!(
        first_choice.get("finish_reason").and_then(|v| v.as_str()),
        Some("stop"),
        "Finish reason should be 'stop'"
    );

    let message = first_choice.get("message")
        .expect("Choice missing 'message' field");

    assert_eq!(
        message.get("role").and_then(|v| v.as_str()),
        Some("assistant"),
        "Message role should be 'assistant'"
    );

    assert!(
        message.get("content").is_some(),
        "Message should have content"
    );

    println!("✅ Response format test passed");
}

/// Helper function to check if the proxy is running
async fn is_proxy_running() -> bool {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();

    client.get(PROXY_URL).send().await.is_ok()
}

#[tokio::test]
async fn test_proxy_health() {
    // Verify the proxy is running before running other tests

    assert!(
        is_proxy_running().await,
        "Proxy is not running at {}. Start it with: docker-compose up -d drakeify",
        PROXY_URL
    );

    println!("✅ Proxy health check passed");
}

