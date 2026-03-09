use anyhow::{Result, Context as AnyhowContext};
use rquickjs::{Context, Function, Runtime};
use std::rc::Rc;
use std::time::Duration;
use std::collections::HashMap;

/// Configuration for JavaScript runtime restrictions
#[derive(Debug, Clone)]
pub struct JsRuntimeConfig {
    pub allow_http: bool,
    pub http_timeout_secs: u64,
    pub http_max_response_size: usize,
    pub allowed_domains: Option<Vec<String>>, // None = allow all
}

impl Default for JsRuntimeConfig {
    fn default() -> Self {
        Self {
            allow_http: true,
            http_timeout_secs: 30,
            http_max_response_size: 10 * 1024 * 1024, // 10MB
            allowed_domains: None, // Allow all by default
        }
    }
}

/// HTTP request options
#[derive(Debug, Clone)]
pub struct HttpRequestOptions {
    pub method: String,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    pub timeout_secs: Option<u64>,
    pub parse_json: bool,
}

/// HTTP response
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub success: bool,
    pub status: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    pub data: String,
    pub error: Option<String>,
}

/// Setup the JavaScript runtime with console, Date, and HTTP support
pub fn setup_js_globals(ctx: &Context, config: &JsRuntimeConfig) -> Result<()> {
    ctx.with(|ctx| {
        // Create a Rust-backed print function that JavaScript can call
        let print_fn = Function::new(ctx.clone(), |msg: String| {
            println!("{}", msg);
        })?;

        ctx.globals().set("__rust_print", print_fn)?;

        // Setup console with JavaScript helper to convert all args to strings
        let console_code = r#"
            // Helper to convert any value to a string
            function valueToString(val) {
                if (val === null) return 'null';
                if (val === undefined) return 'undefined';
                if (typeof val === 'string') return val;
                if (typeof val === 'number') return String(val);
                if (typeof val === 'boolean') return String(val);
                if (typeof val === 'object') {
                    try {
                        return JSON.stringify(val);
                    } catch (e) {
                        return '[object]';
                    }
                }
                return String(val);
            }

            // Helper to format all arguments into a single string
            function formatArgs(prefix, args) {
                var parts = [prefix];
                for (var i = 0; i < args.length; i++) {
                    parts.push(valueToString(args[i]));
                }
                return parts.join(' ');
            }

            // Console implementation that calls Rust print function
            globalThis.console = {
                log: function() {
                    __rust_print(formatArgs('[LOG]', arguments));
                },
                error: function() {
                    __rust_print(formatArgs('[ERROR]', arguments));
                },
                warn: function() {
                    __rust_print(formatArgs('[WARN]', arguments));
                },
                info: function() {
                    __rust_print(formatArgs('[INFO]', arguments));
                },
                debug: function() {
                    __rust_print(formatArgs('[DEBUG]', arguments));
                }
            };
        "#;
        let _: rquickjs::Value = ctx.eval(console_code.as_bytes())?;

        // Setup Date object (QuickJS has built-in Date support, but let's ensure it's available)
        let date_code = r#"
            // QuickJS already has Date, but we can add helper methods if needed
            if (typeof Date === 'undefined') {
                throw new Error('Date is not available in this runtime');
            }
            
            // Add a simple timestamp helper
            globalThis.timestamp = function() {
                return Date.now();
            };
        "#;
        let _: rquickjs::Value = ctx.eval(date_code.as_bytes())?;

        // Setup HTTP fetch function if allowed
        if config.allow_http {
            setup_http_fetch(&ctx, config)?;
        }

        Ok(())
    })
}

/// Perform a comprehensive HTTP request with all options
pub fn http_request_sync(options: HttpRequestOptions, config: &JsRuntimeConfig) -> Result<HttpResponse> {
    // Validate URL
    let parsed_url = url::Url::parse(&options.url)
        .with_context(|| format!("Invalid URL: {}", options.url))?;

    // Check domain whitelist if configured
    if let Some(ref allowed_domains) = config.allowed_domains {
        let host = parsed_url.host_str()
            .ok_or_else(|| anyhow::anyhow!("URL has no host"))?;

        if !allowed_domains.iter().any(|d| host.ends_with(d)) {
            return Ok(HttpResponse {
                success: false,
                status: 0,
                status_text: "Forbidden".to_string(),
                headers: HashMap::new(),
                data: String::new(),
                error: Some(format!("Domain '{}' is not in allowed list", host)),
            });
        }
    }

    // Determine timeout
    let timeout_secs = options.timeout_secs.unwrap_or(config.http_timeout_secs);
    let max_size = config.http_max_response_size;
    let method = options.method.to_uppercase();
    let url = options.url.clone();
    let headers = options.headers.clone();
    let body = options.body.clone();
    let parse_json = options.parse_json;

    // Use tokio's spawn to run the async operation
    let handle = tokio::runtime::Handle::try_current()
        .map_err(|_| anyhow::anyhow!("No tokio runtime available"))?;

    // Create a oneshot channel for the result
    let (tx, rx) = std::sync::mpsc::channel();

    handle.spawn(async move {
        let result = async {
            // Build the HTTP client
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(timeout_secs))
                .build()?;

            // Build the request
            let mut request = match method.as_str() {
                "GET" => client.get(&url),
                "POST" => client.post(&url),
                "PUT" => client.put(&url),
                "DELETE" => client.delete(&url),
                "PATCH" => client.patch(&url),
                "HEAD" => client.head(&url),
                _ => return Err(anyhow::anyhow!("Unsupported HTTP method: {}", method)),
            };

            // Add headers
            for (key, value) in headers {
                request = request.header(&key, &value);
            }

            // Add body if present
            if let Some(body_data) = body {
                request = request.body(body_data);
            }

            // Send the request
            let response = request.send().await
                .with_context(|| format!("Failed to send {} request to {}", method, url))?;

            // Get status
            let status = response.status();
            let status_code = status.as_u16();
            let status_text = status.canonical_reason().unwrap_or("Unknown").to_string();

            // Get headers
            let mut response_headers = HashMap::new();
            for (key, value) in response.headers() {
                if let Ok(value_str) = value.to_str() {
                    response_headers.insert(key.to_string(), value_str.to_string());
                }
            }

            // Check response size
            let content_length = response.content_length().unwrap_or(0);
            if content_length > max_size as u64 {
                return Ok(HttpResponse {
                    success: false,
                    status: status_code,
                    status_text,
                    headers: response_headers,
                    data: String::new(),
                    error: Some(format!(
                        "Response size ({} bytes) exceeds maximum allowed ({} bytes)",
                        content_length, max_size
                    )),
                });
            }

            // Get response body
            let text = response.text().await?;

            // Double-check actual size
            if text.len() > max_size {
                return Ok(HttpResponse {
                    success: false,
                    status: status_code,
                    status_text,
                    headers: response_headers,
                    data: String::new(),
                    error: Some(format!(
                        "Response size ({} bytes) exceeds maximum allowed ({} bytes)",
                        text.len(), max_size
                    )),
                });
            }

            // Parse JSON if requested
            let data = if parse_json {
                // Validate that it's valid JSON
                match serde_json::from_str::<serde_json::Value>(&text) {
                    Ok(_) => text,
                    Err(e) => {
                        return Ok(HttpResponse {
                            success: false,
                            status: status_code,
                            status_text,
                            headers: response_headers,
                            data: text,
                            error: Some(format!("Failed to parse JSON: {}", e)),
                        });
                    }
                }
            } else {
                text
            };

            Ok(HttpResponse {
                success: status.is_success(),
                status: status_code,
                status_text,
                headers: response_headers,
                data,
                error: if status.is_success() { None } else { Some(format!("HTTP {}", status_code)) },
            })
        }.await;

        let _ = tx.send(result);
    });

    // Wait for the result
    rx.recv()
        .map_err(|_| anyhow::anyhow!("HTTP request task failed"))?
}

/// Perform an HTTP GET request with safeguards (legacy function)
/// This function is synchronous but performs async HTTP requests internally
pub fn http_get_sync(url: String, config: &JsRuntimeConfig) -> Result<String> {
    // Validate URL
    let parsed_url = url::Url::parse(&url)
        .with_context(|| format!("Invalid URL: {}", url))?;

    // Check domain whitelist if configured
    if let Some(ref allowed_domains) = config.allowed_domains {
        let host = parsed_url.host_str()
            .ok_or_else(|| anyhow::anyhow!("URL has no host"))?;

        if !allowed_domains.iter().any(|d| host.ends_with(d)) {
            return Err(anyhow::anyhow!("Domain '{}' is not in allowed list", host));
        }
    }

    // Clone config for the async block
    let timeout_secs = config.http_timeout_secs;
    let max_size = config.http_max_response_size;

    // Use tokio's spawn_blocking to run the async operation
    // This avoids the "cannot block_on from within async" error
    let handle = tokio::runtime::Handle::try_current()
        .map_err(|_| anyhow::anyhow!("No tokio runtime available"))?;

    // We need to use a different approach - create a oneshot channel
    let (tx, rx) = std::sync::mpsc::channel();

    handle.spawn(async move {
        let result = async {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(timeout_secs))
                .build()?;

            let response = client.get(&url)
                .send()
                .await
                .with_context(|| format!("Failed to fetch URL: {}", url))?;

            // Check response size
            let content_length = response.content_length().unwrap_or(0);
            if content_length > max_size as u64 {
                return Err(anyhow::anyhow!(
                    "Response size ({} bytes) exceeds maximum allowed ({} bytes)",
                    content_length,
                    max_size
                ));
            }

            let text = response.text().await?;

            // Double-check actual size
            if text.len() > max_size {
                return Err(anyhow::anyhow!(
                    "Response size ({} bytes) exceeds maximum allowed ({} bytes)",
                    text.len(),
                    max_size
                ));
            }

            Ok(text)
        }.await;

        let _ = tx.send(result);
    });

    // Wait for the result
    rx.recv()
        .map_err(|_| anyhow::anyhow!("HTTP request task failed"))?
}

/// Perform an HTTP POST request with safeguards
/// This function is synchronous but performs async HTTP requests internally
pub fn http_post_sync(url: String, body: String, config: &JsRuntimeConfig) -> Result<String> {
    // Validate URL
    let parsed_url = url::Url::parse(&url)
        .with_context(|| format!("Invalid URL: {}", url))?;

    // Check domain whitelist if configured
    if let Some(ref allowed_domains) = config.allowed_domains {
        let host = parsed_url.host_str()
            .ok_or_else(|| anyhow::anyhow!("URL has no host"))?;

        if !allowed_domains.iter().any(|d| host.ends_with(d)) {
            return Err(anyhow::anyhow!("Domain '{}' is not in allowed list", host));
        }
    }

    // Clone config for the async block
    let timeout_secs = config.http_timeout_secs;
    let max_size = config.http_max_response_size;

    // Use tokio's spawn to run the async operation
    let handle = tokio::runtime::Handle::try_current()
        .map_err(|_| anyhow::anyhow!("No tokio runtime available"))?;

    // Create a oneshot channel for the result
    let (tx, rx) = std::sync::mpsc::channel();

    handle.spawn(async move {
        let result = async {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(timeout_secs))
                .build()?;

            let response = client.post(&url)
                .header("Content-Type", "application/json")
                .body(body)
                .send()
                .await
                .with_context(|| format!("Failed to POST to URL: {}", url))?;

            // Check response size
            let content_length = response.content_length().unwrap_or(0);
            if content_length > max_size as u64 {
                return Err(anyhow::anyhow!(
                    "Response size ({} bytes) exceeds maximum allowed ({} bytes)",
                    content_length,
                    max_size
                ));
            }

            let text = response.text().await?;

            // Double-check actual size
            if text.len() > max_size {
                return Err(anyhow::anyhow!(
                    "Response size ({} bytes) exceeds maximum allowed ({} bytes)",
                    text.len(),
                    max_size
                ));
            }

            Ok(text)
        }.await;

        let _ = tx.send(result);
    });

    // Wait for the result
    rx.recv()
        .map_err(|_| anyhow::anyhow!("HTTP request task failed"))?
}

/// Setup HTTP fetch function with safeguards
/// Note: The actual HTTP functions will be injected by tools.rs and plugins.rs
/// This just sets up placeholder functions
fn setup_http_fetch(ctx: &rquickjs::Ctx, _config: &JsRuntimeConfig) -> Result<()> {
    // Setup placeholder JavaScript functions
    // The actual implementation will be injected by the tool/plugin registry
    let http_code = r#"
        // Placeholder HTTP functions - will be replaced by Rust
        globalThis.__rust_http_get = function(url) {
            throw new Error('HTTP not available in this context');
        };

        globalThis.__rust_http_post = function(url, body) {
            throw new Error('HTTP not available in this context');
        };

        globalThis.__rust_http_request = function(optionsJson) {
            throw new Error('HTTP not available in this context');
        };

        // Comprehensive HTTP function
        globalThis.http = function(options) {
            // Validate options
            if (!options || !options.url) {
                return {
                    success: false,
                    status: 0,
                    statusText: 'Bad Request',
                    headers: {},
                    data: null,
                    error: 'URL is required'
                };
            }

            // Build request options
            var requestOptions = {
                method: (options.method || 'GET').toUpperCase(),
                url: options.url,
                headers: options.headers || {},
                body: null,
                timeout: options.timeout,
                parseJson: options.parseJson || false
            };

            // Handle body
            if (options.body !== undefined && options.body !== null) {
                if (typeof options.body === 'string') {
                    requestOptions.body = options.body;
                } else {
                    // Auto-serialize objects to JSON
                    requestOptions.body = JSON.stringify(options.body);
                    if (!requestOptions.headers['Content-Type'] && !requestOptions.headers['content-type']) {
                        requestOptions.headers['Content-Type'] = 'application/json';
                    }
                }
            }

            // Call Rust function
            try {
                var responseJson = __rust_http_request(JSON.stringify(requestOptions));
                return JSON.parse(responseJson);
            } catch (e) {
                return {
                    success: false,
                    status: 0,
                    statusText: 'Error',
                    headers: {},
                    data: null,
                    error: String(e)
                };
            }
        };

        // Add method shortcuts to http object
        globalThis.http.get = function(options) {
            if (typeof options === 'string') {
                options = { url: options };
            }
            options.method = 'GET';
            return globalThis.http(options);
        };

        globalThis.http.post = function(options) {
            if (typeof options === 'string') {
                options = { url: options };
            }
            options.method = 'POST';
            return globalThis.http(options);
        };

        globalThis.http.put = function(options) {
            if (typeof options === 'string') {
                options = { url: options };
            }
            options.method = 'PUT';
            return globalThis.http(options);
        };

        globalThis.http.delete = function(options) {
            if (typeof options === 'string') {
                options = { url: options };
            }
            options.method = 'DELETE';
            return globalThis.http(options);
        };

        globalThis.http.patch = function(options) {
            if (typeof options === 'string') {
                options = { url: options };
            }
            options.method = 'PATCH';
            return globalThis.http(options);
        };

        globalThis.http.request = function(options) {
            return globalThis.http(options);
        };

        // HTTP GET wrapper (legacy, kept for compatibility)
        globalThis.httpGet = function(url) {
            try {
                return {
                    success: true,
                    data: __rust_http_get(url)
                };
            } catch (e) {
                return {
                    success: false,
                    error: String(e)
                };
            }
        };

        // HTTP POST wrapper (legacy, kept for compatibility)
        globalThis.httpPost = function(url, data) {
            try {
                var body = typeof data === 'string' ? data : JSON.stringify(data);
                return {
                    success: true,
                    data: __rust_http_post(url, body)
                };
            } catch (e) {
                return {
                    success: false,
                    error: String(e)
                };
            }
        };

        // Convenience fetch-like API (now uses comprehensive http function)
        globalThis.fetch = function(url, options) {
            options = options || {};
            return http({
                method: options.method || 'GET',
                url: url,
                headers: options.headers,
                body: options.body,
                timeout: options.timeout,
                parseJson: options.parseJson
            });
        };

        // Helper functions
        globalThis.buildQueryString = function(params) {
            if (!params || typeof params !== 'object') return '';
            var parts = [];
            for (var key in params) {
                if (params.hasOwnProperty(key)) {
                    parts.push(encodeURIComponent(key) + '=' + encodeURIComponent(params[key]));
                }
            }
            return parts.length > 0 ? '?' + parts.join('&') : '';
        };

        globalThis.parseJson = function(text) {
            try {
                return { success: true, data: JSON.parse(text), error: null };
            } catch (e) {
                return { success: false, data: null, error: String(e) };
            }
        };

        // Placeholder for get_config - will be replaced by Rust if database is available
        globalThis.__rust_get_config = function(scope) {
            return '{}';
        };

        // Config getter function
        globalThis.get_config = function(scope) {
            try {
                var configJson = __rust_get_config(scope);
                return JSON.parse(configJson);
            } catch (e) {
                console.error('Failed to parse config for scope:', scope, e);
                return {};
            }
        };

        // Placeholder for session functions - will be replaced by Rust if database is available
        globalThis.__rust_get_session = function(sessionId) {
            throw new Error('Session management not available in this context');
        };

        globalThis.__rust_set_session = function(sessionId, sessionDataJson) {
            throw new Error('Session management not available in this context');
        };

        globalThis.__rust_clear_session = function(sessionId) {
            throw new Error('Session management not available in this context');
        };

        // Placeholder for get_current_session_id - will be replaced by Rust
        globalThis.get_current_session_id = function() {
            return '';
        };

        // Session getter function
        globalThis.get_session = function(sessionId) {
            try {
                var resultJson = __rust_get_session(sessionId);

                // Handle null case (session doesn't exist)
                if (resultJson === 'null') {
                    return null;
                }

                var result = JSON.parse(resultJson);

                // Check for error
                if (result && result.__error) {
                    throw new Error(result.__error);
                }

                return result;
            } catch (e) {
                throw new Error('Failed to get session: ' + String(e));
            }
        };

        // Session setter function
        globalThis.set_session = function(sessionId, sessionData) {
            try {
                var sessionDataJson = JSON.stringify(sessionData);
                var resultJson = __rust_set_session(sessionId, sessionDataJson);
                var result = JSON.parse(resultJson);

                // Check for error
                if (result && result.__error) {
                    throw new Error(result.__error);
                }
            } catch (e) {
                throw new Error('Failed to set session: ' + String(e));
            }
        };

        // Session clear function
        globalThis.clear_session = function(sessionId) {
            try {
                var resultJson = __rust_clear_session(sessionId);
                var result = JSON.parse(resultJson);

                // Check for error
                if (result && result.__error) {
                    throw new Error(result.__error);
                }

                return result ? result.deleted : false;
            } catch (e) {
                throw new Error('Failed to clear session: ' + String(e));
            }
        };

        // Placeholder for document store functions - will be replaced by Rust if database is available
        globalThis.__rust_set_document = function(namespace, key, value, metadata) {
            throw new Error('Document store not available in this context');
        };

        globalThis.__rust_get_document = function(namespace, key) {
            throw new Error('Document store not available in this context');
        };

        globalThis.__rust_delete_document = function(namespace, key) {
            throw new Error('Document store not available in this context');
        };

        globalThis.__rust_list_documents = function(namespace) {
            throw new Error('Document store not available in this context');
        };

        // Document store wrapper functions
        // Note: These functions automatically namespace keys based on the tool/plugin name
        // Tools/plugins should call set_document(key, value) and the namespace is handled automatically

        globalThis.set_document = function(key, value, metadata) {
            try {
                // Get the current namespace (injected by tool/plugin registry)
                var namespace = globalThis.__document_namespace || 'default';

                // Convert value to string if it's an object
                var valueStr = typeof value === 'string' ? value : JSON.stringify(value);

                // Convert metadata to string if provided
                var metadataStr = metadata ? (typeof metadata === 'string' ? metadata : JSON.stringify(metadata)) : '{}';

                var resultJson = __rust_set_document(namespace, key, valueStr, metadataStr);
                var result = JSON.parse(resultJson);

                // Check for error
                if (result && result.__error) {
                    throw new Error(result.__error);
                }

                return result ? result.success : false;
            } catch (e) {
                throw new Error('Failed to set document: ' + String(e));
            }
        };

        globalThis.get_document = function(key) {
            try {
                // Get the current namespace (injected by tool/plugin registry)
                var namespace = globalThis.__document_namespace || 'default';

                var resultJson = __rust_get_document(namespace, key);

                // Handle null case
                if (resultJson === 'null') {
                    return null;
                }

                var result = JSON.parse(resultJson);

                // Check for error
                if (result && result.__error) {
                    throw new Error(result.__error);
                }

                // Try to parse value as JSON, otherwise return as string
                if (result && result.value) {
                    try {
                        result.value = JSON.parse(result.value);
                    } catch (e) {
                        // Value is not JSON, keep as string
                    }
                }

                return result;
            } catch (e) {
                throw new Error('Failed to get document: ' + String(e));
            }
        };

        globalThis.delete_document = function(key) {
            try {
                // Get the current namespace (injected by tool/plugin registry)
                var namespace = globalThis.__document_namespace || 'default';

                var resultJson = __rust_delete_document(namespace, key);
                var result = JSON.parse(resultJson);

                // Check for error
                if (result && result.__error) {
                    throw new Error(result.__error);
                }

                return result ? result.deleted : false;
            } catch (e) {
                throw new Error('Failed to delete document: ' + String(e));
            }
        };

        globalThis.list_documents = function() {
            try {
                // Get the current namespace (injected by tool/plugin registry)
                var namespace = globalThis.__document_namespace || 'default';

                var resultJson = __rust_list_documents(namespace);
                var result = JSON.parse(resultJson);

                // Check for error
                if (result && result.__error) {
                    throw new Error(result.__error);
                }

                return result || [];
            } catch (e) {
                throw new Error('Failed to list documents: ' + String(e));
            }
        };

        // Placeholder for LLM function - will be replaced by Rust if LLM config is available
        globalThis.__rust_call_llm = function(optionsJson) {
            throw new Error('LLM not available in this context');
        };

        // LLM caller function
        globalThis.call_llm = function(options) {
            try {
                // Validate options
                if (!options || !options.messages) {
                    throw new Error('options.messages is required');
                }

                var optionsJson = JSON.stringify(options);
                var resultJson = __rust_call_llm(optionsJson);
                var result = JSON.parse(resultJson);

                // Check for error
                if (result.__error) {
                    throw new Error(result.__error);
                }

                return result;
            } catch (e) {
                throw new Error('Failed to call LLM: ' + String(e));
            }
        };

        // Placeholder for process_conversation - will be replaced by Rust if LLM config is available
        globalThis.__rust_process_conversation = function(messagesJson) {
            throw new Error('process_conversation not available in this context');
        };

        // Process conversation function - runs full Drakeify loop with tools and plugins
        globalThis.process_conversation = function(messages) {
            try {
                // Validate messages
                if (!messages || !Array.isArray(messages)) {
                    throw new Error('messages must be an array');
                }

                var messagesJson = JSON.stringify(messages);
                var resultJson = __rust_process_conversation(messagesJson);
                var result = JSON.parse(resultJson);

                // Check for error
                if (result.__error) {
                    throw new Error(result.__error);
                }

                return result;
            } catch (e) {
                throw new Error('Failed to process conversation: ' + String(e));
            }
        };

        // ============================================================================
        // LLM Configuration Management API
        // ============================================================================

        // Placeholder for LLM config functions - will be replaced by Rust if manager is available
        globalThis.__rust_llm_list = function() {
            return '[]';
        };

        globalThis.__rust_llm_get = function(id) {
            return 'null';
        };

        globalThis.__rust_llm_register_selector = function(priority, selectorId) {
            throw new Error('LLM config manager not available in this context');
        };

        // drakeify.llm namespace
        globalThis.drakeify = globalThis.drakeify || {};
        globalThis.drakeify.llm = {
            // List all LLM configurations
            list: function() {
                try {
                    var resultJson = __rust_llm_list();
                    var result = JSON.parse(resultJson);

                    // Check for error
                    if (result && result.__error) {
                        throw new Error(result.__error);
                    }

                    return result || [];
                } catch (e) {
                    throw new Error('Failed to list LLM configs: ' + String(e));
                }
            },

            // Get a specific LLM configuration by ID
            get: function(id) {
                try {
                    if (!id || typeof id !== 'string') {
                        throw new Error('id must be a non-empty string');
                    }

                    var resultJson = __rust_llm_get(id);
                    var result = JSON.parse(resultJson);

                    // Check for error
                    if (result && result.__error) {
                        throw new Error(result.__error);
                    }

                    return result;
                } catch (e) {
                    throw new Error('Failed to get LLM config: ' + String(e));
                }
            },

            // Register a selector function with priority
            // The selector receives a context object and returns an LLM ID or null
            registerSelector: function(selectorFn, priority) {
                try {
                    if (typeof selectorFn !== 'function') {
                        throw new Error('selectorFn must be a function');
                    }

                    // Default priority is 0
                    priority = typeof priority === 'number' ? priority : 0;

                    // Generate a unique ID for this selector
                    var selectorId = 'selector_' + Date.now() + '_' + Math.random().toString(36).substr(2, 9);

                    // Store the selector function globally so Rust can call it
                    globalThis.__llm_selectors = globalThis.__llm_selectors || {};
                    globalThis.__llm_selectors[selectorId] = selectorFn;

                    // Register with Rust
                    __rust_llm_register_selector(priority, selectorId);

                    return selectorId;
                } catch (e) {
                    throw new Error('Failed to register LLM selector: ' + String(e));
                }
            }
        };
    "#;
    let _: rquickjs::Value = ctx.eval(http_code.as_bytes())?;

    Ok(())
}

/// Create a new JavaScript runtime with all globals configured
pub fn create_configured_runtime(config: &JsRuntimeConfig) -> Result<(Rc<Runtime>, Context)> {
    let runtime = Rc::new(Runtime::new()?);
    let context = Context::full(&runtime)?;
    
    setup_js_globals(&context, config)?;
    
    Ok((runtime, context))
}

