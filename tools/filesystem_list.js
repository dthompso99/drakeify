// Filesystem list tool - self-contained tool definition

// Register function - called by Rust to get tool metadata
function register() {
    return {
        name: "filesystem_list",
        description: "List files and directories in a given path",
        parameters: {
            type: "object",
            properties: {
                path: {
                    type: "string",
                    description: "Directory path to list"
                }
            },
            required: ["path"]
        }
    };
}

// Execute function - called when the LLM invokes this tool
function execute(args) {
    const path = args.path || "/";

    // TODO: Call into Rust to actually list the directory
    // For now, return a mock response
    const result = {
        success: true,
        path: path,
        files: [
            "file1.txt",
            "file2.txt",
            "directory1/"
        ],
        message: `Listed files in ${path}`
    };

    return JSON.stringify(result);
}

// Export both functions
({ register, execute })

