# Agency Tools

This directory contains JavaScript-based tool implementations that can be called by the LLM.

## Architecture

Tools are implemented in JavaScript and executed via QuickJS runtime. This provides:
- **Sandboxed execution** - Tools run in an isolated JavaScript environment
- **Easy extensibility** - Add new tools by creating `.js` files
- **Simple interface** - Tools receive JSON args and return JSON results

## Tool Structure

Tools can be implemented in two ways:

### Single Tool (Simple)

A single tool per file with `register()` and `execute()` functions:

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

// Export both functions
({ register, execute })
```

### Multi-Tool Bundle (Advanced)

Multiple related tools in a single file. The `register()` function returns an **array** of tool definitions, and `execute()` uses `args._tool_name` to dispatch to the correct tool:

```javascript
// Register multiple tools
function register() {
    return [
        {
            name: "tool_one",
            description: "First tool",
            parameters: { type: "object", properties: {}, required: [] }
        },
        {
            name: "tool_two",
            description: "Second tool",
            parameters: { type: "object", properties: {}, required: [] }
        }
    ];
}

// Dispatcher that routes to the correct tool
function execute(args) {
    const toolName = args && args._tool_name;

    if (toolName === "tool_one") {
        return JSON.stringify({ success: true, message: "Tool one executed" });
    }

    if (toolName === "tool_two") {
        return JSON.stringify({ success: true, message: "Tool two executed" });
    }

    return JSON.stringify({
        success: false,
        error: `Unknown tool: ${toolName}`
    });
}

// Export both functions
({ register, execute })
```

**Note:** The runtime automatically injects `_tool_name` into the args when executing a tool from a bundle.

## Current Tools

### filesystem_list.js
Lists files in a directory.

**Parameters:**
- `path` (string, required) - Directory path to list

**Returns:**
```json
{
    "success": true,
    "path": "/tmp",
    "files": ["file1.txt", "file2.txt"],
    "message": "Listed files in /tmp"
}
```

### weather.js
Gets weather information for a postal code.

**Parameters:**
- `postal_code` (string, required) - Postal code for location

**Returns:**
```json
{
    "success": true,
    "postal_code": "49709",
    "temperature": 72,
    "conditions": "Partly Cloudy",
    "humidity": 65,
    "message": "Weather for 49709: 72°F, Partly Cloudy"
}
```

### zulip_bundle.js (Multi-Tool Bundle Example)
A demonstration of the multi-tool bundle pattern. Provides three Zulip-related tools in a single file:

- **zulip_list_users** - List all users in the Zulip organization
- **zulip_list_topics** - List all topics in a Zulip stream
- **zulip_send_message** - Send a message to a Zulip stream or user

See the file for implementation details.

## Adding New Tools

1. Create a new `.js` file in this directory
2. Implement the tool function following the structure above
3. Register the tool in `src/main.rs` in the `setup_tools()` function:

```rust
let schema = SchemaBuilder::new()
    .add_string("param1", "Description", true)
    .add_number("param2", "Description", false)
    .build();

let code = std::fs::read_to_string("tools/my_tool.js")?;

registry.register_js_tool(
    "my_tool".to_string(),
    "Tool description".to_string(),
    schema,
    code,
)?;
```

## Future Enhancements

- **Native Rust bindings** - Allow JS tools to call Rust functions for system access
- **Async support** - Enable tools to make async calls
- **Tool discovery** - Auto-register tools from directory
- **Permissions system** - Control what tools can access
- **Tool composition** - Allow tools to call other tools

