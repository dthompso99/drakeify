// Tool Router Plugin
// Intelligently filters tools based on conversation context to reduce LLM context size
// Uses keyword matching on tool descriptions to determine relevance

function register() {
    return {
        name: "tool_router",
        description: "Routes and filters tools based on conversation context to reduce token usage",
        priority: 15, // Run after system_prompt_builder but before context_pruner
        hooks: {
            pre_request: true
        }
    };
}

function pre_request(data) {
    // data: { messages, tools, options }
    
    const tools = data.tools || [];
    const messages = data.messages || [];
    
    // If we have few tools, don't bother filtering
    if (tools.length <= 5) {
        console.log(`[Tool Router] Only ${tools.length} tools, skipping filter`);
        return data;
    }
    
    // Extract conversation context from recent messages
    const recentMessages = messages.slice(-5); // Last 5 messages
    const conversationText = recentMessages
        .map(m => m.content || '')
        .join(' ')
        .toLowerCase();
    
    // Define keyword categories and their associated keywords
    const keywordCategories = {
        web: ['http', 'url', 'website', 'fetch', 'download', 'api', 'request', 'search', 'google', 'duckduckgo', 'internet', 'online', 'web'],
        weather: ['weather', 'temperature', 'forecast', 'climate', 'rain', 'sunny', 'cloudy'],
        filesystem: ['file', 'directory', 'folder', 'path', 'list', 'read', 'write', 'delete'],
        messaging: ['zulip', 'slack', 'message', 'chat', 'send', 'notification', 'dm', 'channel'],
        session: ['session', 'clear', 'reset', 'conversation', 'history'],
        account: ['account', 'user', 'profile', 'info', 'settings']
    };
    
    // Detect which categories are relevant based on conversation
    const relevantCategories = new Set();
    
    for (const category in keywordCategories) {
        const keywords = keywordCategories[category];
        for (const keyword of keywords) {
            if (conversationText.includes(keyword)) {
                relevantCategories.add(category);
                break; // Found a match for this category
            }
        }
    }
    
    console.log(`[Tool Router] Detected categories:`, Array.from(relevantCategories).join(', ') || 'none');
    
    // Filter tools based on relevance
    const filteredTools = tools.filter(tool => {
        const toolName = tool.function?.name || '';
        const toolDescription = (tool.function?.description || '').toLowerCase();
        
        // ALWAYS include client tools (tools that interact with the client/session)
        // These are essential for basic functionality
        const alwaysInclude = [
            'clear_session',
            'get_account_info'
        ];
        
        if (alwaysInclude.includes(toolName)) {
            return true;
        }
        
        // If no categories detected, include all tools (conservative approach)
        if (relevantCategories.size === 0) {
            return true;
        }
        
        // Check if tool description matches any relevant category keywords
        for (const category of relevantCategories) {
            const keywords = keywordCategories[category];
            for (const keyword of keywords) {
                if (toolDescription.includes(keyword) || toolName.toLowerCase().includes(keyword)) {
                    return true;
                }
            }
        }
        
        return false;
    });
    
    const originalCount = tools.length;
    const filteredCount = filteredTools.length;
    const reduction = originalCount - filteredCount;
    
    if (reduction > 0) {
        console.log(`[Tool Router] Filtered tools: ${originalCount} → ${filteredCount} (removed ${reduction})`);
        console.log(`[Tool Router] Kept tools:`, filteredTools.map(t => t.function?.name).join(', '));
    } else {
        console.log(`[Tool Router] No filtering applied (all tools relevant)`);
    }
    
    // Update the tools array
    data.tools = filteredTools;
    
    return data;
}

// Export the functions
({ register, pre_request })

