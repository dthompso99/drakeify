/**
 * Document Store Test Tool
 * 
 * This tool demonstrates the document store functionality.
 * It can set, get, delete, and list documents in the tool's namespace.
 */

({
    name: "document_store_test",
    description: "Test tool for document store functionality. Can set, get, delete, and list documents.",
    parameters: {
        type: "object",
        properties: {
            action: {
                type: "string",
                enum: ["set", "get", "delete", "list"],
                description: "Action to perform: set, get, delete, or list"
            },
            key: {
                type: "string",
                description: "Document key (required for set, get, delete)"
            },
            value: {
                type: "string",
                description: "Document value (required for set)"
            },
            metadata: {
                type: "object",
                description: "Optional metadata for the document (for set action)"
            }
        },
        required: ["action"]
    },
    execute: function(args) {
        try {
            const action = args.action;
            
            if (action === "set") {
                if (!args.key || !args.value) {
                    return JSON.stringify({
                        success: false,
                        error: "key and value are required for set action"
                    });
                }
                
                // Set document with optional metadata
                const success = set_document(args.key, args.value, args.metadata);
                
                return JSON.stringify({
                    success: true,
                    action: "set",
                    key: args.key,
                    message: "Document stored successfully"
                });
            }
            
            if (action === "get") {
                if (!args.key) {
                    return JSON.stringify({
                        success: false,
                        error: "key is required for get action"
                    });
                }
                
                // Get document
                const doc = get_document(args.key);
                
                if (doc === null) {
                    return JSON.stringify({
                        success: true,
                        action: "get",
                        key: args.key,
                        found: false,
                        message: "Document not found"
                    });
                }
                
                return JSON.stringify({
                    success: true,
                    action: "get",
                    key: args.key,
                    found: true,
                    value: doc.value,
                    metadata: doc.metadata,
                    created_at: doc.created_at,
                    updated_at: doc.updated_at
                });
            }
            
            if (action === "delete") {
                if (!args.key) {
                    return JSON.stringify({
                        success: false,
                        error: "key is required for delete action"
                    });
                }
                
                // Delete document
                const deleted = delete_document(args.key);
                
                return JSON.stringify({
                    success: true,
                    action: "delete",
                    key: args.key,
                    deleted: deleted,
                    message: deleted ? "Document deleted successfully" : "Document not found"
                });
            }
            
            if (action === "list") {
                // List all document keys
                const keys = list_documents();
                
                return JSON.stringify({
                    success: true,
                    action: "list",
                    keys: keys,
                    count: keys.length
                });
            }
            
            return JSON.stringify({
                success: false,
                error: "Invalid action: " + action
            });
            
        } catch (e) {
            return JSON.stringify({
                success: false,
                error: String(e)
            });
        }
    }
})

