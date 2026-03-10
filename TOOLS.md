# JavaScript Tool Runtime API Reference

This document describes the complete API available to JavaScript tools running in the QuickJS runtime.

## Table of Contents

1. [Tool Structure](#tool-structure)
2. [Tool Metadata (metadata.json)](#tool-metadata-metadatajson)
3. [Global API](#global-api)
   - [HTTP Functions](#http-functions)
   - [Configuration](#configuration)
   - [Session Management](#session-management)
   - [Document Store](#document-store)
   - [Task Scheduling](#task-scheduling)
   - [Utility Functions](#utility-functions)
4. [Secret Interpolation](#secret-interpolation)
5. [Console Logging](#console-logging)
6. [Built-in Tools](#built-in-tools)

---

## Tool Structure

Every JavaScript tool must export an object with two functions:

```javascript
function register() {
    return {
        name: "tool_name",
        description: "What this tool does",
        parameters: {
            type: "object",
            properties: {
                param1: {
                    type: "string",
                    description: "Description of param1"
                },
                param2: {
                    type: "number",
                    description: "Description of param2"
                }
            },
            required: ["param1"]
        }
    };
}

function execute(args) {
    // args is an object with the parameters
    // Must return a JSON string
    return JSON.stringify({
        success: true,
        result: "Tool output"
    });
}

// Export both functions
({ register, execute })
```

**Important:** The `register()` function defines the tool's schema, while `metadata.json` (described below) provides package-level information like version, author, and configuration requirements.

### Multi-Tool Bundles

A single JavaScript file can register multiple tools by returning an array from `register()`:

```javascript
function register() {
    return [
        {
            name: "tool_one",
            description: "First tool",
            parameters: { /* ... */ }
        },
        {
            name: "tool_two",
            description: "Second tool",
            parameters: { /* ... */ }
        }
    ];
}

function execute(args) {
    // args._tool_name contains which tool was called
    const toolName = args._tool_name;

    if (toolName === "tool_one") {
        // Handle tool_one
    } else if (toolName === "tool_two") {
        // Handle tool_two
    }

    return JSON.stringify({ success: true });
}
```

---

## Tool Metadata (metadata.json)

Every tool package must include a `metadata.json` file in its root directory. This file provides package-level information, configuration requirements, and secret definitions.

### Directory Structure

```
data/tools/
└── my_tool/
    ├── metadata.json    # Package metadata
    └── tool.js          # Tool implementation
```

### Metadata Schema

```json
{
  "type": "tool",
  "name": "tool_name",
  "version": "1.0.0",
  "description": "Brief description of what this tool does",
  "author": "Your Name",
  "license": "MIT",
  "homepage": "https://github.com/user/tool",
  "dependencies": {},
  "drakeify_version": ">=0.1.0",
  "tags": ["category", "feature"],
  "created": "2026-03-10T00:00:00.000000000+00:00",
  "default_config": {
    "setting1": "default_value"
  },
  "config_schema": {
    "setting1": {
      "type": "string",
      "description": "Description of this setting",
      "default": "default_value",
      "required": false
    }
  },
  "secrets_schema": {
    "tool_name.api_key": {
      "description": "API key for the service",
      "required": true
    }
  }
}
```

### Field Descriptions

#### Required Fields

- **`type`** (string): Must be `"tool"` for tools or `"plugin"` for plugins
- **`name`** (string): Unique identifier for the tool (should match the tool name in `register()`)
- **`version`** (string): Semantic version (e.g., "1.0.0", "2.1.3")
- **`description`** (string): Brief description of the tool's purpose
- **`created`** (string): ISO 8601 timestamp of when the tool was created

#### Optional Fields

- **`author`** (string): Tool author name or organization
- **`license`** (string): License identifier (e.g., "MIT", "Apache-2.0")
- **`homepage`** (string): URL to tool documentation or repository
- **`dependencies`** (object): Map of dependency names to version requirements (currently unused)
- **`drakeify_version`** (string): Minimum required version of the proxy (e.g., ">=0.1.0")
- **`tags`** (array of strings): Searchable tags for categorization

#### Configuration Fields

- **`default_config`** (object): Default configuration values applied when the tool is first installed
- **`config_schema`** (object): Schema defining configuration options available to users
- **`secrets_schema`** (object): Schema defining required secrets (API keys, tokens, etc.)

### Configuration Schema Format

Each field in `config_schema` defines a user-configurable setting:

```json
{
  "config_schema": {
    "setting_name": {
      "type": "string|number|boolean|object",
      "description": "Human-readable description",
      "default": "default_value",
      "required": true|false
    }
  }
}
```

**Example (Brave Search):**
```json
{
  "config_schema": {
    "default_count": {
      "type": "number",
      "description": "Default number of search results to return (1-20)",
      "default": 5,
      "required": false
    },
    "default_safesearch": {
      "type": "string",
      "description": "Default safe search setting: 'off', 'moderate', or 'strict'",
      "default": "off",
      "required": false
    }
  }
}
```

### Secrets Schema Format

Each field in `secrets_schema` defines a required secret:

```json
{
  "secrets_schema": {
    "scope.secret_name": {
      "description": "What this secret is used for",
      "required": true|false
    }
  }
}
```

**Naming Convention:** Secrets use the format `<scope>.<name>` where:
- `<scope>` is typically the tool name or service name
- `<name>` is the specific secret (e.g., `api_key`, `token`, `password`)

**Example (Zulip):**
```json
{
  "secrets_schema": {
    "zulip.bot_api_key": {
      "description": "Zulip bot API key",
      "required": true
    }
  }
}
```

**Usage in Code:** Secrets are accessed via interpolation:
```javascript
const apiKey = "${secret.zulip.bot_api_key}";
```

### Complete Example (Brave Search)

```json
{
  "type": "tool",
  "name": "brave_search",
  "version": "1.0.1",
  "description": "Search the web using Brave Search API with configurable defaults",
  "author": "Davin Thompson",
  "dependencies": {},
  "drakeify_version": ">=0.1.0",
  "tags": ["search", "web", "brave", "internet"],
  "config_schema": {
    "default_country": {
      "type": "string",
      "description": "Default country code for search results (e.g., 'us', 'uk')",
      "default": "us",
      "required": false
    },
    "default_count": {
      "type": "number",
      "description": "Default number of search results to return (1-20)",
      "default": 5,
      "required": false
    }
  },
  "secrets_schema": {
    "brave_search.api_key": {
      "description": "Brave Search API key",
      "required": true
    }
  },
  "created": "2026-03-09T00:00:00.000000000+00:00"
}
```

### How Configuration is Used

1. **Installation:** When a tool is installed, `default_config` values are written to the database
2. **Runtime:** Tools call `get_config(scope)` to retrieve their configuration
3. **User Customization:** Users can modify configuration via the web UI or API
4. **Secrets:** Secrets are stored separately and interpolated automatically in HTTP requests and `btoa()`

**Example in tool.js:**
```javascript
function execute(args) {
    // Get configuration
    const configJson = get_config("brave_search");
    const config = JSON.parse(configJson);

    // Use config values with fallbacks
    const count = args.count || config.default_count || 5;
    const country = config.default_country || "us";

    // Use secret via interpolation
    const response = http.request({
        url: "https://api.search.brave.com/res/v1/web/search",
        headers: {
            "X-Subscription-Token": "${secret.brave_search.api_key}"
        }
    });

    return JSON.stringify({ success: true });
}
```

---

## Global API

### HTTP Functions

#### `http.request(options)`

Comprehensive HTTP request function supporting all methods.

**Parameters:**
- `options` (object):
  - `method` (string): HTTP method - "GET", "POST", "PUT", "DELETE", "PATCH", "HEAD"
  - `url` (string): Target URL (supports secret interpolation)
  - `headers` (object, optional): Key-value pairs of HTTP headers (supports secret interpolation)
  - `body` (string, optional): Request body (supports secret interpolation)
  - `timeout` (number, optional): Timeout in seconds (default: 30)
  - `parseJson` (boolean, optional): Whether to validate response as JSON (default: false)

**Returns:** Object with:
- `success` (boolean): Whether the request succeeded
- `status` (number): HTTP status code
- `statusText` (string): HTTP status message
- `headers` (object): Response headers
- `data` (string): Response body
- `error` (string|null): Error message if failed

**Example:**
```javascript
const response = http.request({
    method: "POST",
    url: "https://api.example.com/data",
    headers: {
        "Content-Type": "application/json",
        "Authorization": "Bearer ${secret.api.token}"
    },
    body: JSON.stringify({ key: "value" }),
    parseJson: true
});

if (response.success) {
    const data = JSON.parse(response.data);
    console.log("Response:", data);
} else {
    console.error("Error:", response.error);
}
```

#### `http.get(url)` *(Legacy)*

Simple GET request function.

**Parameters:**
- `url` (string): Target URL (supports secret interpolation)

**Returns:** String containing response body, or "ERROR: ..." on failure

**Example:**
```javascript
const data = http.get("https://api.example.com/data");
if (!data.startsWith("ERROR:")) {
    console.log("Success:", data);
}
```

#### `http.post(url, body)` *(Legacy)*

Simple POST request function.

**Parameters:**
- `url` (string): Target URL (supports secret interpolation)
- `body` (string): Request body (supports secret interpolation)

**Returns:** String containing response body, or "ERROR: ..." on failure

---

### Configuration

#### `get_config(scope)`

Retrieve tool-specific configuration from the database.

**Parameters:**
- `scope` (string): Configuration namespace (typically the tool name)

**Returns:** JSON string containing configuration object, or `"{}"` if not found

**Example:**
```javascript
const configJson = get_config("weather");
const config = JSON.parse(configJson);
const apiKey = config.api_key || "default_key";
```

---

### Session Management

#### `get_current_session_id()`

Get the ID of the current conversation session.

**Returns:** String containing session UUID, or empty string if no session

**Example:**
```javascript
const sessionId = get_current_session_id();
console.log("Current session:", sessionId);
```

#### `clear_session(sessionId)`

Delete a session and all its history.

**Parameters:**
- `sessionId` (string): Session UUID to delete

**Returns:** JSON string with `{ success: boolean, deleted: boolean }` or `{ __error: string }`

**Example:**
```javascript
const result = JSON.parse(clear_session(sessionId));
if (result.success) {
    console.log("Session cleared");
}
```

---

### Document Store

The document store provides persistent key-value storage scoped by namespace and account.

#### `memory.set_document(namespace, key, value, metadata)`

Store a document in the persistent store.

**Parameters:**
- `namespace` (string): Namespace for organizing documents
- `key` (string): Unique key within the namespace
- `value` (string): Document content
- `metadata` (string): JSON string with additional metadata

**Returns:** JSON string with `{ success: boolean }` or `{ __error: string }`

**Example:**
```javascript
const result = JSON.parse(memory.set_document(
    "notes",
    "meeting_2026_03_10",
    "Discussed project timeline",
    JSON.stringify({ tags: ["work", "important"] })
));
```

#### `memory.get_document(namespace, key)`

Retrieve a document from the store.

**Parameters:**
- `namespace` (string): Namespace
- `key` (string): Document key

**Returns:** JSON string with `{ value: string, metadata: object, created_at: string, updated_at: string }`, `"null"` if not found, or `{ __error: string }`

**Example:**
```javascript
const docJson = memory.get_document("notes", "meeting_2026_03_10");
if (docJson !== "null") {
    const doc = JSON.parse(docJson);
    console.log("Content:", doc.value);
    console.log("Tags:", doc.metadata.tags);
}
```

#### `memory.delete_document(namespace, key)`

Delete a document from the store.

**Parameters:**
- `namespace` (string): Namespace
- `key` (string): Document key

**Returns:** JSON string with `{ deleted: boolean }` or `{ __error: string }`

**Example:**
```javascript
const result = JSON.parse(memory.delete_document("notes", "old_note"));
if (result.deleted) {
    console.log("Document deleted");
}
```

#### `memory.list_documents(namespace)`

List all document keys in a namespace.

**Parameters:**
- `namespace` (string): Namespace to list

**Returns:** JSON array of key strings, or `{ __error: string }`

**Example:**
```javascript
const keysJson = memory.list_documents("notes");
const keys = JSON.parse(keysJson);
if (!keys.__error) {
    keys.forEach(key => console.log("Found:", key));
}
```

---

### Task Scheduling

#### `schedule_task(prompt, run_at, context)`

Schedule a task to run at a future time. The task will execute with the same tools and context as the current session.

**Parameters:**
- `prompt` (string): Task description/instruction
- `run_at` (string): ISO 8601 timestamp in UTC (e.g., "2026-03-10T15:00:00Z")
- `context` (string): JSON string with additional context

**Returns:** JSON string with `{ success: boolean, job_id: string }` or `{ __error: string }`

**Example:**
```javascript
const result = JSON.parse(schedule_task(
    "Check the weather and send a Zulip message if it will rain",
    "2026-03-11T08:00:00Z",
    JSON.stringify({ note: "Morning weather check" })
));

if (result.success) {
    console.log("Scheduled job:", result.job_id);
}
```

**Note:** This function is exposed via the `scheduler` tool and uses `__rust_schedule_task` internally.

---

### Utility Functions

#### `btoa(string)`

Base64 encode a string. **Automatically performs secret interpolation** before encoding.

**Parameters:**
- `string` (string): String to encode (supports secret interpolation)

**Returns:** Base64-encoded string

**Example:**
```javascript
// Encode credentials with secret interpolation
const encoded = btoa("user:${secret.api.password}");
const authHeader = "Basic " + encoded;
```

#### `atob(string)`

Base64 decode a string.

**Parameters:**
- `string` (string): Base64-encoded string

**Returns:** Decoded string

**Example:**
```javascript
const decoded = atob("SGVsbG8gV29ybGQ=");
console.log(decoded); // "Hello World"
```

#### `get_account_id()`

Get the current account ID (read-only).

**Returns:** String containing account UUID

**Example:**
```javascript
const accountId = get_account_id();
console.log("Account:", accountId);
```

---

## Secret Interpolation

The runtime automatically replaces `${secret.scope.name}` patterns in strings before they leave the JavaScript sandbox. This works in:

- HTTP request URLs
- HTTP request headers
- HTTP request bodies
- `btoa()` input

**Format:** `${secret.<scope>.<name>}`

**Example:**
```javascript
// Secrets are interpolated automatically
const response = http.request({
    method: "GET",
    url: "https://api.example.com/data",
    headers: {
        "Authorization": "Bearer ${secret.api.token}",
        "X-API-Key": "${secret.api.key}"
    }
});

// Also works in btoa
const auth = btoa("${secret.api.username}:${secret.api.password}");
```

**Security:** Secrets are stored in the database and never exposed to JavaScript. The interpolation happens in Rust after the JavaScript execution completes.

---

## Console Logging

The runtime provides a full `console` object for debugging:

```javascript
console.log("Info message", { data: 123 });
console.error("Error message");
console.warn("Warning message");
console.info("Info message");
```

All console output is prefixed with `[LOG]`, `[ERROR]`, `[WARN]`, or `[INFO]` and printed to stdout.

**Object Serialization:** Objects are automatically JSON-stringified:
```javascript
console.log("User:", { id: 1, name: "Alice" });
// Output: [LOG] User: {"id":1,"name":"Alice"}
```

---

## Built-in Tools

### Weather Tool

**Name:** `get_weather`

**Description:** Get weather forecast for a location using OpenStreetMap Nominatim for geocoding and National Weather Service API.

**Parameters:**
- `location` (string): City name or address

**Example Response:**
```json
{
    "location": "New York, NY",
    "forecast": "Tonight: Clear. Low around 45. Tomorrow: Sunny. High near 68."
}
```

**Implementation:** Uses `http.get()` to call:
1. Nominatim API for geocoding
2. NWS Points API for grid coordinates
3. NWS Forecast API for weather data

---

### Memory Tool

**Name:** `memory`

**Description:** Multi-tool bundle for persistent document storage.

**Sub-tools:**
- `memory_set` - Store a document
- `memory_get` - Retrieve a document
- `memory_delete` - Delete a document
- `memory_list` - List all documents in a namespace

**Parameters:** Vary by sub-tool (see Document Store section above)

---

### Zulip Tool

**Name:** `zulip`

**Description:** Multi-tool bundle for Zulip messaging integration.

**Configuration Required:**
```json
{
    "api_url": "https://your-org.zulipchat.com/api/v1",
    "email": "bot@example.com",
    "api_key": "your_api_key"
}
```

**Sub-tools:**
- `zulip_send_message` - Send a message to a stream or user
- `zulip_get_messages` - Retrieve recent messages
- `zulip_get_streams` - List available streams
- `zulip_get_users` - List users

**Example:**
```javascript
// Send a message
{
    "type": "stream",
    "to": "general",
    "topic": "Updates",
    "content": "Hello from the bot!"
}
```

---

### Brave Search Tool

**Name:** `brave_search`

**Description:** Web search using Brave Search API.

**Configuration Required:**
```json
{
    "api_key": "your_brave_api_key"
}
```

**Parameters:**
- `query` (string): Search query
- `count` (number, optional): Number of results (default: 10, max: 20)

**Example Response:**
```json
{
    "success": true,
    "results": [
        {
            "title": "Page Title",
            "url": "https://example.com",
            "description": "Page description..."
        }
    ]
}
```

---

### Scheduler Tool

**Name:** `schedule_task`

**Description:** Schedule a task to run at a future time.

**Parameters:**
- `prompt` (string): Task instruction
- `run_at` (string): ISO 8601 timestamp
- `context` (object, optional): Additional context

**Example:**
```javascript
{
    "prompt": "Send a reminder about the meeting",
    "run_at": "2026-03-11T09:00:00Z",
    "context": {
        "note": "Team standup"
    }
}
```

---

### Clear Session Tool

**Name:** `clear_session`

**Description:** Clear the current conversation session history.

**Parameters:** None

**Example Response:**
```json
{
    "success": true,
    "message": "Session cleared successfully"
}
```

---

## Runtime Limits

- **HTTP Timeout:** 30 seconds (configurable)
- **Max Response Size:** 10 MB (configurable)
- **Domain Whitelist:** Optional (can restrict HTTP to specific domains)
- **Execution Context:** Each tool execution gets a fresh JavaScript context

---

## Error Handling

Always wrap tool execution in try-catch and return JSON:

```javascript
function execute(args) {
    try {
        // Tool logic here
        return JSON.stringify({
            success: true,
            result: "data"
        });
    } catch (e) {
        return JSON.stringify({
            success: false,
            error: String(e)
        });
    }
}
```

**Rust Error Convention:** Functions that return JSON strings use `__error` field for errors:
```javascript
const result = JSON.parse(memory.get_document("ns", "key"));
if (result.__error) {
    console.error("Database error:", result.__error);
}
```

---

## Internal Functions (Advanced)

These functions are used internally by the wrapper code and should not be called directly:

- `__rust_print(msg)` - Low-level print function
- `__rust_http_get(url)` - Legacy HTTP GET
- `__rust_http_post(url, body)` - Legacy HTTP POST
- `__rust_http_request(optionsJson)` - HTTP request implementation
- `__rust_get_config(scope)` - Config retrieval implementation
- `__rust_get_session(sessionId)` - Session retrieval
- `__rust_set_session(sessionId, dataJson)` - Session storage
- `__rust_clear_session(sessionId)` - Session deletion
- `__rust_schedule_task(prompt, runAt, contextJson)` - Task scheduling
- `__rust_set_document(namespace, key, value, metadata)` - Document storage
- `__rust_get_document(namespace, key)` - Document retrieval
- `__rust_delete_document(namespace, key)` - Document deletion
- `__rust_list_documents(namespace)` - Document listing

**Note:** Use the high-level wrappers (`http`, `memory`, etc.) instead of calling these directly.
