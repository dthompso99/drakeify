// Context Pruner Plugin
// Removes old messages to keep conversation under token limit

function register() {
    return {
        name: "context_pruner",
        description: "Prunes old messages to stay under token limit",
        priority: 20, // Run after prompt builder but before sending
        hooks: {
            pre_request: true
        }
    };
}

function pre_request(data) {
    // data: { messages, tools, options }
    
    const MAX_MESSAGES = 20; // Keep last 20 messages (plus system message)
    
    if (data.messages.length <= MAX_MESSAGES) {
        // No pruning needed
        return data;
    }
    
    // Separate system message from conversation messages
    const systemMessages = data.messages.filter(m => m.role === "system");
    const conversationMessages = data.messages.filter(m => m.role !== "system");
    
    // Keep only the most recent messages
    const recentMessages = conversationMessages.slice(-MAX_MESSAGES);
    
    // Reconstruct messages array: system messages first, then recent conversation
    data.messages = [...systemMessages, ...recentMessages];
    
    console.log(`[Context Pruner] Pruned to ${data.messages.length} messages`);
    
    return data;
}

// Export the functions
({ register, pre_request })

