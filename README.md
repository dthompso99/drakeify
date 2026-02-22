# Drakeify

Drakeify is a model-agnostic control plane that transparently adds tool and plugin capabilities to any LLM.

## Features

- **Dual Mode Operation**
  - Interactive CLI mode for direct conversation
  - HTTP proxy mode with OpenAI-compatible API (`/v1/chat/completions`)
  
- **JavaScript Tool System**
  - Self-contained tools with embedded metadata
  - Auto-discovery from `tools/` directory
  - Built-in HTTP client support
  - Enable/disable tools via configuration

- **Plugin Architecture**
  - 6 lifecycle hooks: `pre_request`, `post_response`, `on_stream_chunk`, `on_conversation_turn`, `on_tool_call`, `on_tool_result`
  - Self-contained JavaScript plugins
  - Auto-discovery from `plugins/` directory
  - Enable/disable plugins via configuration

- **Plugin/Tool Management** 🆕
  - Publish plugins and tools to OCI registries
  - Install community plugins and tools with a single command
  - Version management and distribution
  - See [PLUGIN_MANAGEMENT.md](PLUGIN_MANAGEMENT.md) for details

- **Session Management**
  - Persistent conversation history
  - Auto-save support
  - Session metadata and tagging

- **Client Tool Passthrough** (Proxy Mode)
  - Drakeify tools execute transparently behind the scenes
  - Client tools are returned for client-side execution
  - Seamless integration with tools from clients like Open WebUI

## Architecture

Drakeify consists of two binaries:

- **`drakeify`** - HTTP proxy server (headless mode)
- **`drakeify-cli`** - Interactive CLI, plugin/tool management, and shell compatibility

## Quick Start

### Build

```bash
cargo build --release
```

This builds both binaries:
- `target/release/drakeify` - Proxy server
- `target/release/drakeify-cli` - CLI tool

### Configuration

Copy the example configuration:

```bash
cp drakeify.toml.example drakeify.toml
```

Edit `drakeify.toml` to configure your LLM endpoint and preferences.

### Run

**Interactive CLI Mode:**
```bash
./target/release/drakeify-cli
# or
./target/release/drakeify-cli chat
```

**Proxy Mode:**
```bash
./target/release/drakeify
```

The proxy server will start on the configured port (default: 8080).

**Plugin/Tool Management:**
```bash
# Publish a plugin
./target/release/drakeify-cli publish --package-type plugin --path ./my-plugin --name my-plugin --version 1.0.0 --description "My plugin"

# Install a plugin
./target/release/drakeify-cli install --package-type plugin --name my-plugin --version 1.0.0

# List available packages
./target/release/drakeify-cli list --package-type plugin
```

### Docker

**Build:**
```bash
docker build -t drakeify .
```

**Run Proxy:**
```bash
docker run -p 8080:8080 \
  -e DRAKEIFY_LLM_HOST=http://host.docker.internal:11434 \
  -e DRAKEIFY_LLM_MODEL=llama3.1:latest \
  -v $(pwd)/tools:/tools:ro \
  -v $(pwd)/plugins:/plugins:ro \
  drakeify
```

The Docker image includes both binaries:
- `/drakeify` - Proxy server (default CMD)
- `/drakeify-cli` - CLI tool
- `/bin/sh` - Symlinked to drakeify-cli for k9s shell compatibility

**Docker Compose:**
```bash
docker-compose up
```

See `docker-compose.yml` for configuration options.

## Configuration

Configuration can be set via `drakeify.toml` file or environment variables. **Environment variables take precedence** over the config file, making it ideal for containerized deployments.

### Configuration File

Key configuration options in `drakeify.toml`:

- `llm_host` - LLM server URL (e.g., `http://localhost:11434`)
- `llm_model` - Model to use (e.g., `llama3.1:latest`)
- `headless` - Enable proxy mode (`true`) or CLI mode (`false`)
- `proxy_port` - HTTP server port for proxy mode
- `enabled_tools` - Whitelist of tools to load (empty = all)
- `disabled_tools` - Blacklist of tools to skip
- `enabled_plugins` - Whitelist of plugins to load (empty = all)
- `disabled_plugins` - Blacklist of plugins to skip

See `drakeify.toml.example` for all available options.

### Environment Variables

All configuration options can be overridden with environment variables using the `DRAKEIFY_` prefix:

```bash
# Override LLM settings
export DRAKEIFY_LLM_HOST=http://localhost:11434
export DRAKEIFY_LLM_MODEL=llama3.1:latest

# Override proxy settings
export DRAKEIFY_HEADLESS=true
export DRAKEIFY_PROXY_PORT=8080
export DRAKEIFY_PROXY_HOST=0.0.0.0

# Override system prompt
export DRAKEIFY_SYSTEM_PROMPT="You are a helpful assistant"

# Override logging
export DRAKEIFY_LOG_LEVEL=debug
```

This is especially useful for Docker deployments:

```bash
docker run -e DRAKEIFY_LLM_HOST=http://host.docker.internal:11434 \
           -e DRAKEIFY_PROXY_PORT=8080 \
           drakeify
```

## Creating Tools

Tools are self-contained JavaScript files in the `tools/` directory:

```javascript
function register() {
    return {
        name: "my_tool",
        description: "Does something useful",
        parameters: {
            type: "object",
            properties: {
                input: { type: "string", description: "Input parameter" }
            },
            required: ["input"]
        }
    };
}

function execute(args) {
    const result = {
        success: true,
        data: `Processed: ${args.input}`
    };
    return JSON.stringify(result);
}
```

**Important:** Tools must return JSON strings (use `JSON.stringify()`), not objects.

### Built-in HTTP Support

Tools have access to HTTP functions:

```javascript
// GET request
const response = http.get("https://api.example.com/data");

// POST request
const response = http.post("https://api.example.com/data", {
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ key: "value" })
});

// Full control
const response = http.request({
    url: "https://api.example.com/data",
    method: "POST",
    headers: { "Authorization": "Bearer token" },
    body: "request body"
});
```

## Creating Plugins

Plugins are self-contained JavaScript files in the `plugins/` directory:

```javascript
function register() {
    return {
        name: "my_plugin",
        description: "Modifies requests/responses",
        hooks: ["pre_request", "post_response"]
    };
}

function pre_request(messages, tools) {
    // Modify messages or tools before sending to LLM
    return { messages, tools };
}

function post_response(content, tool_calls) {
    // Modify LLM response
    return { content, tool_calls };
}
```

### Available Hooks

- `pre_request(messages, tools)` - Before LLM request
- `post_response(content, tool_calls)` - After LLM response
- `on_stream_chunk(role, content, index)` - During streaming
- `on_conversation_turn(messages)` - After each turn
- `on_tool_call(tool_name, args)` - Before tool execution
- `on_tool_result(tool_name, result)` - After tool execution

## Proxy Mode

When `headless = true`, Drakeify runs as an OpenAI-compatible HTTP server:

```bash
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama3.1:latest",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'
```

Compatible with OpenAI client libraries and tools like Open WebUI.

## License

MIT

