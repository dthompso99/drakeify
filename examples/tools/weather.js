// Weather tool - self-contained tool definition

// Register function - called by Rust to get tool metadata
function register() {
    return {
        name: "weather",
        description: "Get current weather information for a location by postal code",
        parameters: {
            type: "object",
            properties: {
                postal_code: {
                    type: "string",
                    description: "Postal code for the location"
                }
            },
            required: ["postal_code"]
        }
    };
}

// Execute function - called when the LLM invokes this tool
function execute(args) {
    const postalCode = args.postal_code;

    // TODO: Call into Rust to make actual weather API calls
    // For now, return a mock response
    const result = {
        success: true,
        postal_code: postalCode,
        temperature: 72,
        conditions: "Partly Cloudy",
        humidity: 65,
        message: `Weather for ${postalCode}: 72°F, Partly Cloudy`
    };

    return JSON.stringify(result);
}

// Export both functions
({ register, execute })

