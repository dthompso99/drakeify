// HTTP GET Tool
// Fetches data from a URL using HTTP GET

function register() {
    return {
        name: "http_get",
        description: "Fetch data from a URL using HTTP GET request",
        parameters: {
            type: "object",
            properties: {
                url: {
                    type: "string",
                    description: "The URL to fetch data from"
                }
            },
            required: ["url"]
        }
    };
}

function execute(args) {
    var url = args.url;
    
    console.log("[http_get] Fetching:", url);
    
    // Use the httpGet function provided by the runtime
    var result = httpGet(url);
    
    if (result.success) {
        console.log("[http_get] Success! Received", result.data.length, "bytes");
        return JSON.stringify({
            success: true,
            url: url,
            data: result.data,
            size: result.data.length
        });
    } else {
        console.error("[http_get] Error:", result.error);
        return JSON.stringify({
            success: false,
            url: url,
            error: result.error
        });
    }
}

// Export the functions
({ register, execute })

