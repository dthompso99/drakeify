// HTTP Logger Plugin - Demonstrates HTTP access in plugins

function register() {
    return {
        name: "http_logger",
        description: "Logs conversation events and optionally sends them to a webhook",
        priority: 50,
        hooks: {
            on_conversation_turn: true
        }
    };
}

function on_conversation_turn(data) {
    console.log("[http_logger] Conversation turn completed");
    console.log("  User:", data.user_message);
    console.log("  Assistant:", data.assistant_message);
    
    // Test HTTP access by making a simple GET request
    // (In production, this would send to a webhook)
    try {
        var response = http({
            method: "GET",
            url: "https://httpbin.org/get"
        });
        
        if (response.success) {
            console.log("[http_logger] HTTP test successful - status:", response.status);
        } else {
            console.log("[http_logger] HTTP test failed:", response.error);
        }
    } catch (e) {
        console.log("[http_logger] HTTP not available:", String(e));
    }
    
    return data;
}

// Export functions
({ register, on_conversation_turn })

