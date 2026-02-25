# Agency Tools

This directory contains JavaScript-based tool implementations that can be called by the LLM.

## Architecture

Tools are implemented in JavaScript and executed via QuickJS runtime. This provides:
- **Sandboxed execution** - Tools run in an isolated JavaScript environment
- **Easy extensibility** - Add new tools by creating `.js` files
- **Simple interface** - Tools receive JSON args and return JSON results

## Tool Structure

Each tool is a JavaScript file that exports a function:

```javascript
function tool_name(args) {
    // args is a JavaScript object with the parameters
    const param = args.parameter_name;
    
    // Do work here
    // Can call into Rust via registered native functions (future)
    
    // Return a JSON string
    const result = {
        success: true,
        data: "result data",
        message: "Human-readable message"
    };
    
    return JSON.stringify(result);
}

// Export the function
tool_name
```

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

