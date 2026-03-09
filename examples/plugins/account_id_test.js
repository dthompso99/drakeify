// Account ID Test Plugin
// Tests the get_account_id and set_account_id functions

function register() {
    return {
        name: "account_id_test",
        description: "Test plugin for account_id functionality",
        priority: 50,
        hooks: {
            pre_request: true,
            on_webhook_call: true
        }
    };
}

function pre_request(data) {
    // Get the current account_id
    const accountId = get_account_id();
    console.log("[account_id_test] Current account_id:", accountId);
    
    // Add account_id to the first message as a note
    if (data.messages && data.messages.length > 0) {
        const firstMsg = data.messages[0];
        if (firstMsg.role === "user") {
            firstMsg.content = firstMsg.content + "\n\n[Account: " + accountId + "]";
        }
    }
    
    return data;
}

function on_webhook_call(data) {
    const payload = data.payload;
    
    // Get initial account_id
    const initialAccountId = get_account_id();
    console.log("[account_id_test] Webhook received with initial account_id:", initialAccountId);
    
    // If payload has a user_id or email, use it as the account_id
    if (payload.user_id) {
        console.log("[account_id_test] Setting account_id to:", payload.user_id);
        set_account_id(payload.user_id);
    } else if (payload.email) {
        console.log("[account_id_test] Setting account_id to:", payload.email);
        set_account_id(payload.email);
    }
    
    // Verify the change
    const newAccountId = get_account_id();
    console.log("[account_id_test] New account_id:", newAccountId);
    
    return {
        status: "success",
        initial_account_id: initialAccountId,
        new_account_id: newAccountId,
        payload_received: payload
    };
}

// Export the functions
({ register, pre_request, on_webhook_call })

