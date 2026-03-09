/**
 * Memory Tool
 * 
 * Allows the LLM to store and retrieve important thoughts, facts, or context
 * that should be remembered across conversations.
 * 
 * Memories are stored per-account and persist indefinitely until explicitly deleted.
 */

({
    name: "memory",
    description: "Store and retrieve important thoughts, facts, or context. Use this to remember things the user tells you that might be important later. Memories are persistent across conversations.",
    parameters: {
        type: "object",
        properties: {
            action: {
                type: "string",
                enum: ["store", "recall", "list", "forget"],
                description: "Action to perform: 'store' to save a memory, 'recall' to retrieve a specific memory, 'list' to see all memory keys, 'forget' to delete a memory"
            },
            key: {
                type: "string",
                description: "Memory key/identifier (required for store, recall, forget). Use descriptive keys like 'user_name', 'favorite_color', 'project_context', etc."
            },
            thought: {
                type: "string",
                description: "The thought/fact to remember (required for store action)"
            }
        },
        required: ["action"]
    },
    execute: function(args) {
        try {
            const action = args.action;
            
            // STORE: Save a new memory
            if (action === "store") {
                if (!args.key || !args.thought) {
                    return JSON.stringify({
                        success: false,
                        error: "Both 'key' and 'thought' are required for store action"
                    });
                }
                
                // Store the memory with timestamp metadata
                const metadata = {
                    stored_at: new Date().toISOString(),
                    type: "memory"
                };
                
                set_document(args.key, args.thought, metadata);
                
                return JSON.stringify({
                    success: true,
                    action: "store",
                    key: args.key,
                    message: "Memory stored successfully: " + args.key
                });
            }
            
            // RECALL: Retrieve a specific memory
            if (action === "recall") {
                if (!args.key) {
                    return JSON.stringify({
                        success: false,
                        error: "'key' is required for recall action"
                    });
                }
                
                const memory = get_document(args.key);
                
                if (memory === null) {
                    return JSON.stringify({
                        success: true,
                        action: "recall",
                        key: args.key,
                        found: false,
                        message: "No memory found with key: " + args.key
                    });
                }
                
                return JSON.stringify({
                    success: true,
                    action: "recall",
                    key: args.key,
                    found: true,
                    thought: memory.value,
                    stored_at: memory.metadata.stored_at || "unknown",
                    message: "Memory recalled: " + memory.value
                });
            }
            
            // LIST: Show all memory keys
            if (action === "list") {
                const keys = list_documents();
                
                return JSON.stringify({
                    success: true,
                    action: "list",
                    keys: keys,
                    count: keys.length,
                    message: keys.length > 0 
                        ? "Found " + keys.length + " stored memories: " + keys.join(", ")
                        : "No memories stored yet"
                });
            }
            
            // FORGET: Delete a memory
            if (action === "forget") {
                if (!args.key) {
                    return JSON.stringify({
                        success: false,
                        error: "'key' is required for forget action"
                    });
                }
                
                const deleted = delete_document(args.key);
                
                return JSON.stringify({
                    success: true,
                    action: "forget",
                    key: args.key,
                    deleted: deleted,
                    message: deleted 
                        ? "Memory forgotten: " + args.key
                        : "No memory found with key: " + args.key
                });
            }
            
            return JSON.stringify({
                success: false,
                error: "Invalid action: " + action + ". Must be one of: store, recall, list, forget"
            });
            
        } catch (e) {
            return JSON.stringify({
                success: false,
                error: "Memory tool error: " + String(e)
            });
        }
    }
})

