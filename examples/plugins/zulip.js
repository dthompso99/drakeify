// Advanced Zulip Integration Plugin
// Demonstrates session management and full Drakeify integration from webhooks
//
// Configuration (set via database):
// {
//   "zulip_url": "https://your-zulip-domain.zulipchat.com",
//   "bot_email": "bot-name-bot@your-zulip-domain.zulipchat.com"
// }
//
// Secrets (set via database):
// - zulip.bot_api_key: Your Zulip bot's API key
//
// To set configuration:
//   INSERT INTO plugin_configs (plugin_name, config) VALUES ('zulip', '{"zulip_url":"https://...","bot_email":"..."}');
//
// To set secret:
//   INSERT INTO secrets (key, value) VALUES ('zulip.bot_api_key', 'your-api-key-here');

function register() {
    return {
        name: "zulip",
        description: "Advanced Zulip integration with session management and full Drakeify loop",
        priority: 50,
        hooks: {
            on_webhook_call: true,
            post_response: true
        }
    };
}

function on_webhook_call(data) {
    try {
        console.log("[Zulip] Received webhook:", JSON.stringify(data, null, 2));

        // Extract Zulip message data
        const payload = data.payload || {};
        const message = payload.message || {};
        const sender_email = message.sender_email || "unknown@example.com";
        const content = message.content || "";
        const message_type = message.type || "stream"; // "stream" or "private"

        // Skip messages from the bot itself
        if (sender_email.includes("-bot@")) {
            console.log("[Zulip] Skipping bot message");
            return { skip: true };
        }

        // Set account_id to the sender's email for proper session isolation
        set_account_id(sender_email);
        console.log("[Zulip] Set account_id to:", sender_email);

        // Build session ID based on conversation context
        let session_id;
        if (message_type === "private") {
            // For private messages, use sender email
            session_id = "zulip:private:" + sender_email;
        } else {
            // For stream messages, use stream + topic
            const stream = message.stream || "general";
            const topic = message.subject || "general";
            session_id = "zulip:stream:" + stream + ":" + topic;
        }

        console.log("[Zulip] Using session_id:", session_id);

        // Get or create session
        let session;
        try {
            session = get_session(session_id);
            if (!session) {
                console.log("[Zulip] Creating new session");
                session = {
                    messages: [],
                    metadata: {
                        title: message_type === "private"
                            ? "Private chat with " + sender_email
                            : "Stream: " + (message.stream || "general") + " / " + (message.subject || "general"),
                        tags: ["zulip", message_type]
                    }
                };
            } else {
                console.log("[Zulip] Loaded existing session with", session.messages.length, "messages");
            }
        } catch (e) {
            console.error("[Zulip] Failed to get session:", String(e));
            session = {
                messages: [],
                metadata: { title: "Zulip conversation", tags: ["zulip"] }
            };
        }

        // Add user message to session
        session.messages.push({
            role: "user",
            content: content
        });

        console.log("[Zulip] Calling Drakeify with", session.messages.length, "messages");

        // Store webhook context in session metadata for post_response hook
        session.metadata._webhook_context = {
            session_id: session_id,
            sender_email: sender_email,
            message_type: message_type,
            stream: message.stream,
            topic: message.subject
        };

        // IMPORTANT: Save session WITH webhook context BEFORE calling process_conversation
        // so that the nested post_response hook can find it
        console.log("[Zulip] About to save session with metadata:", JSON.stringify(session.metadata, null, 2));
        console.log("[Zulip] Current account_id before save:", get_account_id());
        set_session(session_id, session);
        console.log("[Zulip] Session saved with webhook context");

        // Verify it was saved correctly
        const verify_session = get_session(session_id);
        console.log("[Zulip] Verification - loaded session metadata:", JSON.stringify(verify_session.metadata, null, 2));

        // Call full Drakeify loop (tools, plugins, etc.)
        try {
            const response = process_conversation(session.messages);
            console.log("[Zulip] Drakeify response:", response.content.substring(0, 100) + "...");

            // Add assistant response to session
            session.messages.push({
                role: "assistant",
                content: response.content
            });

            // Save updated session again (now with assistant response, webhook context will be cleaned by post_response)
            set_session(session_id, session);
            console.log("[Zulip] Session saved with assistant response");

            // Return success (actual response will be sent in post_response hook)
            return {
                success: true,
                message: "Processing complete",
                session_id: session_id  // Pass session_id to post_response
            };
        } catch (e) {
            console.error("[Zulip] Drakeify call failed:", String(e));
            return {
                error: "Failed to process conversation: " + String(e)
            };
        }

    } catch (e) {
        console.error("[Zulip] Error in webhook handler:", String(e));
        return {
            error: String(e)
        };
    }
}



function post_response(data) {
    try {
        console.log("[Zulip] ========================================");
        console.log("[Zulip] post_response hook triggered!");
        console.log("[Zulip] Data received:", JSON.stringify(data, null, 2));
        console.log("[Zulip] ========================================");

        // Get current account_id to find the right session
        const account_id = get_account_id();
        console.log("[Zulip] Current account_id:", account_id);

        if (!account_id || !account_id.startsWith("user")) {
            // Not a Zulip user account
            console.log("[Zulip] Not a Zulip user account, skipping");
            return data;
        }

        // Try to find a session with webhook context for this account
        // We'll check both private and stream session patterns
        const private_session_id = "zulip:private:" + account_id;

        let session = null;
        let session_id = null;

        // Try private session first
        try {
            console.log("[Zulip] Trying to load session:", private_session_id);
            session = get_session(private_session_id);
            console.log("[Zulip] Loaded session:", session ? "exists" : "null");
            if (session) {
                console.log("[Zulip] Session metadata:", JSON.stringify(session.metadata, null, 2));
            }
            if (session && session.metadata && session.metadata._webhook_context) {
                session_id = private_session_id;
            }
        } catch (e) {
            console.error("[Zulip] Error loading session:", String(e));
        }

        if (!session || !session.metadata || !session.metadata._webhook_context) {
            console.log("[Zulip] No webhook context found for account:", account_id);
            console.log("[Zulip] Session exists?", !!session);
            console.log("[Zulip] Metadata exists?", !!(session && session.metadata));
            console.log("[Zulip] Webhook context exists?", !!(session && session.metadata && session.metadata._webhook_context));
            return data;
        }

        const context = session.metadata._webhook_context;
        console.log("[Zulip] Found webhook context for session:", session_id);

        // Clear webhook context from metadata
        delete session.metadata._webhook_context;
        set_session(session_id, session);

        // Get configuration
        const config = get_config("zulip");
        if (!config || !config.zulip_url || !config.bot_email) {
            console.error("[Zulip] Missing configuration. Please set zulip_url and bot_email in plugin_configs table.");
            console.log("[Zulip] Response would be:", data.content);
            return data;
        }

        // Build Zulip message payload as form data
        // Zulip API expects application/x-www-form-urlencoded, not JSON!
        const form_params = {
            content: data.content
        };

        if (context.message_type === "private") {
            form_params.type = "private";
            form_params.to = JSON.stringify([context.sender_email]);
        } else {
            form_params.type = "stream";
            form_params.to = context.stream;
            form_params.topic = context.topic;
        }

        console.log("[Zulip] Sending message to Zulip:", JSON.stringify(form_params, null, 2));

        // Convert to URL-encoded form data
        const form_body = Object.keys(form_params)
            .map(key => encodeURIComponent(key) + '=' + encodeURIComponent(form_params[key]))
            .join('&');

        // Send to Zulip API
        // The ${secret.zulip.bot_api_key} will be automatically interpolated by the HTTP layer
        const api_url = config.zulip_url + "/api/v1/messages";
        const auth_string = config.bot_email + ":${secret.zulip.bot_api_key}";
        const auth_header = "Basic " + btoa(auth_string);

        try {
            const response = http.post({
                url: api_url,
                headers: {
                    "Authorization": auth_header,
                    "Content-Type": "application/x-www-form-urlencoded"
                },
                body: form_body
            });

            console.log("[Zulip] Message sent successfully:", response);
        } catch (http_error) {
            console.error("[Zulip] Failed to send message to Zulip:", String(http_error));
        }

        return data;
    } catch (e) {
        console.error("[Zulip] Error in post_response hook:", String(e));
        return data;
    }
}
