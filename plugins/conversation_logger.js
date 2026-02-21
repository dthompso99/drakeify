// Conversation Logger Plugin
// Logs all conversation turns, tool calls, and results

function register() {
    return {
        name: "conversation_logger",
        description: "Logs all conversation activity to console",
        priority: 90, // Run late (high priority number)
        hooks: {
            on_conversation_turn: true,
            on_tool_call: true,
            on_tool_result: true
        }
    };
}

function on_conversation_turn(data) {
    // data: { user_message, assistant_message }
    console.log("=== Conversation Turn ===");
    console.log("User:", data.user_message);
    console.log("Assistant:", data.assistant_message);
    return data;
}

function on_tool_call(data) {
    // data: { tool_name, arguments }
    console.log("🔧 Tool Call:", data.tool_name);
    console.log("Arguments:", data.arguments);
    return data;
}

function on_tool_result(data) {
    // data: { tool_name, result }
    console.log("✅ Tool Result:", data.tool_name);
    console.log("Result:", data.result);
    return data;
}

// Export the functions
({ register, on_conversation_turn, on_tool_call, on_tool_result })

