({
    register: function() {
        return {
            name: "clear_session",
            description: "Clear the current conversation session to start fresh",
            parameters: {
                type: "object",
                properties: {},
                required: []
            }
        };
    },
    
    execute: function(args) {
        // Get the current session ID - tool can ONLY clear its own session
        const session_id = get_current_session_id();
        
        if (!session_id) {
            return JSON.stringify({
                success: false,
                message: "No active session to clear"
            });
        }
        
        const deleted = clear_session(session_id);
        
        if (deleted) {
            return JSON.stringify({
                success: true,
                message: "Session cleared successfully. Starting fresh conversation."
            });
        } else {
            return JSON.stringify({
                success: false,
                message: "Session not found or already empty."
            });
        }
    }
})