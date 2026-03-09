// Zulip multi-tool bundle - demonstrates multiple tools in one file
// This file exports three tools: zulip_list_users, zulip_list_topics, zulip_send_message

// ============================================================================
// Tool 1: List Users
// ============================================================================

function register_list_users() {
    return {
        name: "zulip_list_users",
        description: "List all users in the Zulip organization",
        parameters: {
            type: "object",
            properties: {
                include_bots: {
                    type: "boolean",
                    description: "Whether to include bot users in the list"
                }
            },
            required: []
        }
    };
}

function execute_list_users(args) {
    const includeBots = args.include_bots || false;
    
    // TODO: Make actual API call to Zulip
    // For now, return mock data
    const users = [
        { id: 1, name: "Alice", email: "alice@example.com", is_bot: false },
        { id: 2, name: "Bob", email: "bob@example.com", is_bot: false },
        { id: 3, name: "Notification Bot", email: "bot@example.com", is_bot: true }
    ];
    
    const filteredUsers = includeBots ? users : users.filter(u => !u.is_bot);
    
    return JSON.stringify({
        success: true,
        users: filteredUsers,
        count: filteredUsers.length,
        message: `Found ${filteredUsers.length} user(s)`
    });
}

// ============================================================================
// Tool 2: List Topics
// ============================================================================

function register_list_topics() {
    return {
        name: "zulip_list_topics",
        description: "List all topics in a Zulip stream",
        parameters: {
            type: "object",
            properties: {
                stream_name: {
                    type: "string",
                    description: "Name of the stream to list topics from"
                }
            },
            required: ["stream_name"]
        }
    };
}

function execute_list_topics(args) {
    const streamName = args.stream_name;
    
    if (!streamName) {
        return JSON.stringify({
            success: false,
            error: "stream_name is required"
        });
    }
    
    // TODO: Make actual API call to Zulip
    // For now, return mock data
    const topics = [
        { name: "general", message_count: 42 },
        { name: "announcements", message_count: 15 },
        { name: "help", message_count: 8 }
    ];
    
    return JSON.stringify({
        success: true,
        stream: streamName,
        topics: topics,
        count: topics.length,
        message: `Found ${topics.length} topic(s) in stream '${streamName}'`
    });
}

// ============================================================================
// Tool 3: Send Message
// ============================================================================

function register_send_message() {
    return {
        name: "zulip_send_message",
        description: "Send a message to a Zulip stream or user",
        parameters: {
            type: "object",
            properties: {
                type: {
                    type: "string",
                    description: "Message type: 'stream' or 'private'",
                    enum: ["stream", "private"]
                },
                to: {
                    type: "string",
                    description: "Stream name (for stream messages) or user email (for private messages)"
                },
                topic: {
                    type: "string",
                    description: "Topic name (required for stream messages)"
                },
                content: {
                    type: "string",
                    description: "Message content"
                }
            },
            required: ["type", "to", "content"]
        }
    };
}

function execute_send_message(args) {
    const { type, to, topic, content } = args;
    
    if (!type || !to || !content) {
        return JSON.stringify({
            success: false,
            error: "type, to, and content are required"
        });
    }
    
    if (type === "stream" && !topic) {
        return JSON.stringify({
            success: false,
            error: "topic is required for stream messages"
        });
    }
    
    // TODO: Make actual API call to Zulip
    // For now, return mock success
    return JSON.stringify({
        success: true,
        message_id: 12345,
        message: `Message sent to ${type === "stream" ? "stream" : "user"}

// ============================================================================
// Multi-tool Bundle Registration and Execution
// ============================================================================

// Register function returns an array of tool definitions
function register() {
    return [
        register_list_users(),
        register_list_topics(),
        register_send_message()
    ];
}

// Execute function dispatches to the appropriate tool based on _tool_name
function execute(args) {
    // The runtime injects _tool_name into args to identify which tool to execute
    const toolName = args && args._tool_name;

    if (toolName === "zulip_list_users") {
        return execute_list_users(args);
    }

    if (toolName === "zulip_list_topics") {
        return execute_list_topics(args);
    }

    if (toolName === "zulip_send_message") {
        return execute_send_message(args);
    }

    // If we get here, the tool name is unknown
    return JSON.stringify({
        success: false,
        error: `Unknown tool: ${toolName}. Expected one of: zulip_list_users, zulip_list_topics, zulip_send_message`
    });
}

// Export both functions
({ register, execute }) '${to}'${topic ? ` in topic '${topic}'` : ""}`
    });
}

