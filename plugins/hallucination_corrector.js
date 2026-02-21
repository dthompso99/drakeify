// Hallucination Corrector Plugin
// Detects and corrects common LLM hallucinations

function register() {
    return {
        name: "hallucination_corrector",
        description: "Detects and corrects common LLM hallucinations in responses",
        priority: 80, // Run late, after most processing
        hooks: {
            post_response: true
        }
    };
}

function post_response(data) {
    // data: { content, tool_calls }
    
    // Example: Remove common hallucination patterns
    // In a real implementation, this would be more sophisticated
    let corrected_content = data.content;
    
    // Remove phrases like "I don't have access to real-time data" when we actually do have tools
    if (data.tool_calls && data.tool_calls.length > 0) {
        corrected_content = corrected_content.replace(
            /I don't have access to real-time (data|information)/gi,
            "Based on the available tools"
        );
    }
    
    // Return modified data
    return {
        content: corrected_content,
        tool_calls: data.tool_calls
    };
}

// Export the functions
({ register, post_response })

