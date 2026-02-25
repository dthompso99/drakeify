// System Prompt Builder Plugin
// Enhances the system prompt with additional context and instructions

function register() {
    return {
        name: "system_prompt_builder",
        description: "Enhances system prompts with additional context",
        priority: 10, // Run early (low priority number)
        hooks: {
            pre_request: true
        }
    };
}

function pre_request(data) {
    // data: { messages, tools, options }
    
    // Find the system message (first message with role "system")
    const systemMessageIndex = data.messages.findIndex(m => m.role === "system");
    
    if (systemMessageIndex !== -1) {
        const currentPrompt = data.messages[systemMessageIndex].content;
        
        // Add additional instructions to the system prompt
        const enhancements = [
            "\n\n## Additional Guidelines:",
            "- Be concise and direct in your responses",
            "- When using tools, explain what you're doing",
            "- If you're unsure, ask for clarification",
            "- Format code blocks with proper syntax highlighting"
        ].join("\n");
        
        // Append enhancements to existing system prompt
        data.messages[systemMessageIndex].content = currentPrompt + enhancements;
    }
    
    return data;
}

// Export the functions
({ register, pre_request })

