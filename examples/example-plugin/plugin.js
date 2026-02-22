// Example Plugin
// This is a simple example plugin that demonstrates the plugin structure

function register() {
    return {
        name: "example_plugin",
        description: "An example plugin that logs requests",
        priority: 50,
        hooks: {
            pre_request: true,
            post_response: true
        }
    };
}

function pre_request(data) {
    // data: { messages, tools }
    console.log("[example_plugin] Pre-request hook called");
    console.log("[example_plugin] Number of messages:", data.messages.length);
    console.log("[example_plugin] Number of tools:", data.tools.length);
    
    // Return unmodified data
    return data;
}

function post_response(data) {
    // data: { content, tool_calls }
    console.log("[example_plugin] Post-response hook called");
    console.log("[example_plugin] Response content length:", data.content ? data.content.length : 0);
    console.log("[example_plugin] Number of tool calls:", data.tool_calls.length);
    
    // Return unmodified data
    return data;
}

// Export the functions
({ register, pre_request, post_response })

