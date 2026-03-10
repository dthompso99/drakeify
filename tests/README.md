# Drakeify Integration Tests

This directory contains integration tests for the Drakeify proxy server.

## Overview

The integration tests verify end-to-end functionality of the proxy, including:

- ✅ **Request Parsing**: Ensures the proxy correctly parses OpenAI-compatible chat completion requests
- ✅ **Model Routing**: Verifies that dynamic LLM configuration works and requests are routed to the correct model
- ✅ **Response Format**: Confirms responses follow the OpenAI API format
- ✅ **Conversation Context**: Tests that multi-turn conversations maintain context
- ✅ **Tool Availability**: Verifies that tools are available to the LLM
- ✅ **Authentication**: Ensures auth tokens are required
- ✅ **Error Handling**: Tests graceful handling of invalid requests

## Running the Tests

### Quick Start

```bash
# Make sure the proxy is running
docker-compose up -d drakeify

# Run all integration tests
./run_integration_tests.sh
```

### Manual Test Execution

```bash
# Run all tests
cargo test --test proxy_integration_test

# Run a specific test
cargo test --test proxy_integration_test test_simple_chat_completion

# Run with output
cargo test --test proxy_integration_test -- --nocapture

# Run tests sequentially (recommended for integration tests)
cargo test --test proxy_integration_test -- --test-threads=1
```

## Prerequisites

1. **Proxy Running**: The proxy must be running at `http://localhost:8082`
   ```bash
   docker-compose up -d drakeify
   ```

2. **LLM Available**: An LLM must be configured and accessible
   - Either via environment variables (fallback)
   - Or via database configuration (preferred)

3. **Web UI Running** (optional): For managing LLM configs
   ```bash
   docker-compose up -d drakeify-web
   ```

## Test Configuration

The tests use these default values:

- **Proxy URL**: `http://localhost:8082`
- **Auth Token**: `change_me_in_production`
- **Timeout**: 30 seconds per request

To change these, edit `tests/proxy_integration_test.rs`.

## Troubleshooting

### Tests Fail with "Connection Refused"

The proxy is not running. Start it with:
```bash
docker-compose up -d drakeify
docker-compose logs drakeify
```

### Tests Fail with "Request Timeout"

The LLM may be slow or not responding. Check:
```bash
# Check if Ollama is running
curl http://localhost:11434/api/tags

# Check proxy logs
docker-compose logs drakeify --tail=50
```

### Tests Fail with "Model is 'default'"

Dynamic LLM configuration may not be working. Verify:
```bash
# Check LLM configs in database
curl -H "Authorization: Bearer change_me_in_production" \
  http://localhost:3974/api/llm/configs | jq .

# Check proxy is using the right config
docker-compose logs drakeify | grep -i "select"
```

## Adding New Tests

To add a new test:

1. Add a new `#[tokio::test]` function to `tests/proxy_integration_test.rs`
2. Use the `send_chat_request()` helper for making requests
3. Add assertions to verify expected behavior
4. Run the test to verify it works

Example:
```rust
#[tokio::test]
async fn test_my_new_feature() {
    let messages = vec![
        json!({
            "role": "user",
            "content": "Test my feature"
        })
    ];

    let response = send_chat_request(messages, None)
        .await
        .expect("Failed to send request");

    // Add assertions here
    assert!(response.get("choices").is_some());
    
    println!("✅ My new feature test passed");
}
```

## CI/CD Integration

To run these tests in CI/CD:

```bash
# Start services
docker-compose up -d

# Wait for services to be ready
sleep 5

# Run tests
cargo test --test proxy_integration_test -- --test-threads=1

# Cleanup
docker-compose down
```

## Test Coverage

Current test coverage:

| Category | Tests | Status |
|----------|-------|--------|
| Basic Functionality | 3 | ✅ |
| Model Routing | 1 | ✅ |
| Authentication | 1 | ✅ |
| Error Handling | 1 | ✅ |
| Response Format | 1 | ✅ |
| Tool Integration | 1 | ✅ |

**Total**: 8 tests

