// Simple webhook test plugin
// Used to test webhook functionality

function register() {
    return {
        name: "webhook_test",
        description: "Simple webhook test plugin",
        priority: 50,
        hooks: {
            on_webhook_call: true
        }
    };
}

function on_webhook_call(data) {
    // data: { payload }
    console.log("🪝 Webhook test received");
    console.log("Payload:", JSON.stringify(data.payload, null, 2));
    
    // Echo back the payload with some additional info
    return {
        status: "success",
        message: "Webhook received successfully",
        received_payload: data.payload,
        timestamp: new Date().toISOString()
    };
}

// Export the functions
({ register, on_webhook_call })

