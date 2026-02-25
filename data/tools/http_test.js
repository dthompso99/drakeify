// Comprehensive HTTP test tool demonstrating all HTTP features

function register() {
    return {
        name: "http_test",
        description: "Test comprehensive HTTP functionality with various methods, headers, and options",
        parameters: {
            type: "object",
            properties: {
                test_type: {
                    type: "string",
                    description: "Type of test to run: 'get', 'post', 'put', 'delete', 'headers', 'json', 'status'",
                    enum: ["get", "post", "put", "delete", "headers", "json", "status"]
                }
            },
            required: ["test_type"]
        }
    };
}

function execute(args) {
    var testType = args.test_type;
    console.log("[http_test] Running test:", testType);

    var result;
    try {
        switch (testType) {
            case "get":
                result = testGet();
                break;
            case "post":
                result = testPost();
                break;
            case "put":
                result = testPut();
                break;
            case "delete":
                result = testDelete();
                break;
            case "headers":
                result = testHeaders();
                break;
            case "json":
                result = testJson();
                break;
            case "status":
                result = testStatus();
                break;
            default:
                result = {
                    success: false,
                    error: "Unknown test type: " + testType
                };
        }
    } catch (e) {
        result = {
            success: false,
            error: "Test failed: " + String(e)
        };
    }

    return JSON.stringify(result);
}

function testGet() {
    console.log("Testing HTTP GET...");
    var response = http({
        method: "GET",
        url: "https://httpbin.org/get"
    });
    
    return {
        test: "GET",
        success: response.success,
        status: response.status,
        statusText: response.statusText,
        hasData: response.data && response.data.length > 0
    };
}

function testPost() {
    console.log("Testing HTTP POST...");
    var response = http({
        method: "POST",
        url: "https://httpbin.org/post",
        body: { message: "Hello from Agency!", timestamp: Date.now() }
    });
    
    return {
        test: "POST",
        success: response.success,
        status: response.status,
        statusText: response.statusText,
        hasData: response.data && response.data.length > 0
    };
}

function testPut() {
    console.log("Testing HTTP PUT...");
    var response = http({
        method: "PUT",
        url: "https://httpbin.org/put",
        body: { updated: true }
    });
    
    return {
        test: "PUT",
        success: response.success,
        status: response.status,
        statusText: response.statusText
    };
}

function testDelete() {
    console.log("Testing HTTP DELETE...");
    var response = http({
        method: "DELETE",
        url: "https://httpbin.org/delete"
    });
    
    return {
        test: "DELETE",
        success: response.success,
        status: response.status,
        statusText: response.statusText
    };
}

function testHeaders() {
    console.log("Testing custom headers...");
    var response = http({
        method: "GET",
        url: "https://httpbin.org/headers",
        headers: {
            "X-Custom-Header": "Agency-Test",
            "User-Agent": "Agency/1.0"
        }
    });
    
    return {
        test: "Custom Headers",
        success: response.success,
        status: response.status,
        responseHeaders: response.headers
    };
}

function testJson() {
    console.log("Testing JSON parsing...");
    var response = http({
        method: "GET",
        url: "https://httpbin.org/json",
        parseJson: true
    });
    
    return {
        test: "JSON Parsing",
        success: response.success,
        status: response.status,
        dataType: typeof response.data
    };
}

function testStatus() {
    console.log("Testing status codes...");
    var response = http({
        method: "GET",
        url: "https://httpbin.org/status/404"
    });

    return {
        test: "Status Codes",
        success: response.success,
        status: response.status,
        statusText: response.statusText,
        error: response.error
    };
}

// Export both functions
({ register, execute })

