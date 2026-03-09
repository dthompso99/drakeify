// Get Account Info Tool
// Returns information about the current account

function register() {
    return {
        name: "get_account_info",
        description: "Get information about the current account/user making the request",
        parameters: {
            type: "object",
            properties: {},
            required: []
        }
    };
}

function execute(args) {
    // Get the account_id
    const accountId = get_account_id();
    
    console.log("[get_account_info] Account ID:", accountId);
    
    const result = {
        success: true,
        account_id: accountId,
        is_anonymous: accountId === "anonymous",
        timestamp: new Date().toISOString()
    };
    
    return JSON.stringify(result);
}

// Export the tool
({ register, execute })

