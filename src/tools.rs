use anyhow::{Context, Result};
use rquickjs::{Context as JsContext, Runtime, Value as JsValue, Object, Function};
use serde_json::Value;
use std::collections::{HashMap, BTreeMap};
use std::fs;
use std::path::Path;

use crate::llm::{JsonSchema, OllamaFunction, OllamaFunctionDefinition};
use crate::js_runtime::{JsRuntimeConfig, setup_js_globals, http_get_sync, http_post_sync, http_request_sync, HttpRequestOptions};
use crate::database::Database;
use tracing::warn;

/// Interpolate secrets in a string (synchronous version for use in closures)
/// Replaces ${secret.scope.name} with the actual secret value from the database
async fn interpolate_secrets_sync(text: &str, database: &Database) -> String {
    let mut result = text.to_string();

    // Find all ${secret.scope.name} patterns
    let re = regex::Regex::new(r"\$\{secret\.([^}]+)\}").unwrap();

    for cap in re.captures_iter(text) {
        let full_match = &cap[0];
        let secret_key = &cap[1];

        // Get the secret value from database
        match database.get_secret(secret_key).await {
            Ok(Some(value)) => {
                result = result.replace(full_match, &value);
            }
            Ok(None) => {
                warn!("Secret not found: {}", secret_key);
            }
            Err(e) => {
                warn!("Failed to get secret {}: {}", secret_key, e);
            }
        }
    }

    result
}

pub struct Tool {
    pub name: String,
    pub description: String,
    pub schema: JsonSchema,
    js_code: String,
}

pub struct ToolRegistry {
    tools: HashMap<String, Tool>,
    js_runtime: Runtime,
    config: JsRuntimeConfig,
    enabled_tools: Option<Vec<String>>,
    disabled_tools: Option<Vec<String>>,
    database: Option<std::sync::Arc<Database>>,
    account_id: String,
    session_id: Option<String>,
}

impl ToolRegistry {
    /// Create a new tool registry with a QuickJS runtime
    pub fn new(config: JsRuntimeConfig, enabled_tools: Option<Vec<String>>, disabled_tools: Option<Vec<String>>, database: Option<std::sync::Arc<Database>>, account_id: Option<String>) -> Result<Self> {
        let js_runtime = Runtime::new()?;

        // Setup globals in the runtime
        let ctx = JsContext::full(&js_runtime)?;
        setup_js_globals(&ctx, &config)?;

        Ok(Self {
            tools: HashMap::new(),
            js_runtime,
            config,
            enabled_tools,
            disabled_tools,
            database,
            account_id: account_id.unwrap_or_else(|| "anonymous".to_string()),
            session_id: None,
        })
    }

    /// Set the current session ID for this tool registry
    pub fn set_session_id(&mut self, session_id: Option<String>) {
        self.session_id = session_id;
    }

    /// Check if a tool should be loaded based on enabled/disabled lists
    fn should_load_tool(&self, tool_name: &str) -> bool {
        // If enabled_tools is specified, only load tools in that list
        if let Some(ref enabled) = self.enabled_tools {
            return enabled.contains(&tool_name.to_string());
        }

        // If disabled_tools is specified, don't load tools in that list
        if let Some(ref disabled) = self.disabled_tools {
            return !disabled.contains(&tool_name.to_string());
        }

        // By default, load all tools
        true
    }

    /// Auto-discover and register all tools from a directory
    pub fn load_tools_from_dir<P: AsRef<Path>>(&mut self, dir: P) -> Result<()> {
        let dir_path = dir.as_ref();

        if !dir_path.exists() {
            return Err(anyhow::anyhow!("Tools directory does not exist: {:?}", dir_path));
        }

        for entry in fs::read_dir(dir_path)? {
            let entry = entry?;
            let path = entry.path();

            // Check if it's a .js file directly in the directory
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("js") {
                let js_code = fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read tool file: {:?}", path))?;

                self.register_tool_from_js(&js_code)
                    .with_context(|| format!("Failed to register tool from: {:?}", path))?;
            }
            // Check if it's a directory with a tool.js file (installed package format)
            else if path.is_dir() {
                let tool_file = path.join("tool.js");
                if tool_file.exists() {
                    let js_code = fs::read_to_string(&tool_file)
                        .with_context(|| format!("Failed to read tool file: {:?}", tool_file))?;

                    self.register_tool_from_js(&js_code)
                        .with_context(|| format!("Failed to register tool from: {:?}", tool_file))?;
                }
            }
        }

        Ok(())
    }

    /// Register a tool by calling its register() function
    /// Supports both single tool registration and multi-tool bundles (arrays)
    fn register_tool_from_js(&mut self, js_code: &str) -> Result<()> {
        let ctx = JsContext::full(&self.js_runtime)?;

        ctx.with(|ctx| {
            // Evaluate the tool code
            let tool_obj: Object = ctx.eval(js_code.as_bytes())?;

            // Call the register() function
            let register_fn: rquickjs::Function = tool_obj.get("register")?;
            let result: JsValue = register_fn.call(())?;

            // Check if result is an array (multi-tool bundle) or object (single tool)
            if result.is_array() {
                // Multi-tool bundle: iterate over array and register each tool
                // Convert to Object to access array properties
                let array_obj: Object = result.into_object().context("Failed to convert array to object")?;
                let array_len: usize = array_obj.get("length")?;

                for i in 0..array_len {
                    // Convert index to u32 which implements IntoAtom
                    let metadata: Object = array_obj.get(i as u32)?;
                    self.register_single_tool_from_metadata(&ctx, metadata, js_code)?;
                }
            } else if result.is_object() {
                // Single tool: register it directly
                let metadata: Object = result.into_object().context("Expected object from register()")?;
                self.register_single_tool_from_metadata(&ctx, metadata, js_code)?;
            } else {
                anyhow::bail!("register() must return an object or array of objects");
            }

            Ok(())
        })
    }

    /// Register a single tool from its metadata object
    fn register_single_tool_from_metadata<'js>(&mut self, ctx: &rquickjs::Ctx<'js>, metadata: Object<'js>, js_code: &str) -> Result<()> {
        // Extract metadata
        let name: String = metadata.get("name")?;

        // Check if this tool should be loaded
        if !self.should_load_tool(&name) {
            return Ok(());
        }

        let description: String = metadata.get("description")?;
        let params_obj: Object = metadata.get("parameters")?;

        // Convert parameters to JsonSchema
        let schema = self.parse_schema_from_js(ctx, params_obj)?;

        // Create and register the tool
        let tool = Tool {
            name: name.clone(),
            description,
            schema,
            js_code: js_code.to_string(),
        };

        self.tools.insert(name, tool);
        Ok(())
    }

    /// Parse a JSON schema from a JavaScript object
    fn parse_schema_from_js<'js>(&self, ctx: &rquickjs::Ctx<'js>, obj: Object<'js>) -> Result<JsonSchema> {
        // Convert JS object to JSON string, then parse it
        let json_str: String = ctx.json_stringify(obj)?.unwrap().to_string()?;
        let value: Value = serde_json::from_str(&json_str)?;

        // Convert to JsonSchema
        self.value_to_schema(value)
    }

    /// Convert a serde_json::Value to JsonSchema
    fn value_to_schema(&self, value: Value) -> Result<JsonSchema> {
        let obj = value.as_object()
            .context("Schema must be an object")?;

        let schema_type = obj.get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("object")
            .to_string();

        let description = obj.get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let properties = obj.get("properties")
            .and_then(|v| v.as_object())
            .map(|props| {
                props.iter()
                    .filter_map(|(k, v)| {
                        self.value_to_schema(v.clone()).ok()
                            .map(|schema| (k.clone(), schema))
                    })
                    .collect::<BTreeMap<_, _>>()
            });

        let required = obj.get("required")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            });

        Ok(JsonSchema {
            schema_type,
            description,
            properties,
            required,
        })
    }

    /// Check if a tool exists in the registry
    pub fn has_tool(&self, tool_name: &str) -> bool {
        self.tools.contains_key(tool_name)
    }

    /// Execute a tool by name with the given arguments
    pub fn execute(&self, tool_name: &str, args: Value) -> Result<Value> {
        let tool = self.tools.get(tool_name)
            .context(format!("Tool '{}' not found", tool_name))?;

        self.execute_js_tool(&tool.js_code, tool_name, args)
    }

    /// Execute a JavaScript tool by calling its execute() function
    /// Injects _tool_name into args to support multi-tool bundles
    fn execute_js_tool(&self, js_code: &str, tool_name: &str, args: Value) -> Result<Value> {
        let ctx = JsContext::full(&self.js_runtime)?;

        // Setup globals for this execution context
        setup_js_globals(&ctx, &self.config)?;

        // Inject HTTP functions if allowed
        if self.config.allow_http {
            ctx.with(|ctx| {
                // Legacy GET function
                let config_clone = self.config.clone();
                let database_clone = self.database.clone();
                let http_get_fn = Function::new(ctx.clone(), move |url: String| -> String {
                    let mut final_url = url;

                    // Interpolate secrets if database is available
                    if let Some(ref db) = database_clone {
                        if let Some(h) = tokio::runtime::Handle::try_current().ok() {
                            let db_clone = db.clone();
                            let url_clone = final_url.clone();
                            final_url = tokio::task::block_in_place(|| {
                                h.block_on(async move {
                                    interpolate_secrets_sync(&url_clone, &db_clone).await
                                })
                            });
                        }
                    }

                    match http_get_sync(final_url, &config_clone) {
                        Ok(data) => data,
                        Err(e) => {
                            // Return error as a string that JS can handle
                            format!("ERROR: {}", e)
                        }
                    }
                })?;
                ctx.globals().set("__rust_http_get", http_get_fn)?;

                // Legacy POST function
                let config_clone2 = self.config.clone();
                let database_clone2 = self.database.clone();
                let http_post_fn = Function::new(ctx.clone(), move |url: String, body: String| -> String {
                    let mut final_url = url;
                    let mut final_body = body;

                    // Interpolate secrets if database is available
                    if let Some(ref db) = database_clone2 {
                        if let Some(h) = tokio::runtime::Handle::try_current().ok() {
                            let db_clone = db.clone();
                            let url_clone = final_url.clone();
                            final_url = tokio::task::block_in_place(|| {
                                h.block_on(async move {
                                    interpolate_secrets_sync(&url_clone, &db_clone).await
                                })
                            });

                            let db_clone2 = db.clone();
                            let body_clone = final_body.clone();
                            final_body = tokio::task::block_in_place(|| {
                                h.block_on(async move {
                                    interpolate_secrets_sync(&body_clone, &db_clone2).await
                                })
                            });
                        }
                    }

                    match http_post_sync(final_url, final_body, &config_clone2) {
                        Ok(data) => data,
                        Err(e) => {
                            // Return error as a string that JS can handle
                            format!("ERROR: {}", e)
                        }
                    }
                })?;
                ctx.globals().set("__rust_http_post", http_post_fn)?;

                // Comprehensive HTTP request function
                let config_clone3 = self.config.clone();
                let database_clone = self.database.clone();
                let http_request_fn = Function::new(ctx.clone(), move |options_json: String| -> String {
                    // Parse options from JSON
                    let options_value: serde_json::Value = match serde_json::from_str(&options_json) {
                        Ok(v) => v,
                        Err(e) => {
                            return serde_json::json!({
                                "success": false,
                                "status": 0,
                                "statusText": "Bad Request",
                                "headers": {},
                                "data": null,
                                "error": format!("Failed to parse options: {}", e)
                            }).to_string();
                        }
                    };

                    // Build HttpRequestOptions
                    let mut url = options_value.get("url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let mut headers: HashMap<String, String> = options_value.get("headers")
                        .and_then(|v| v.as_object())
                        .map(|obj| {
                            obj.iter()
                                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                                .collect()
                        })
                        .unwrap_or_default();
                    let mut body = options_value.get("body")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    // Interpolate secrets if database is available
                    if let Some(ref db) = database_clone {
                        // Use blocking call to interpolate secrets
                        let handle = tokio::runtime::Handle::try_current().ok();
                        if let Some(h) = handle {
                            let db_clone = db.clone();
                            let url_clone = url.clone();
                            url = tokio::task::block_in_place(|| {
                                h.block_on(async move {
                                    interpolate_secrets_sync(&url_clone, &db_clone).await
                                })
                            });

                            // Interpolate in headers
                            let headers_clone = headers.clone();
                            let db_clone2 = db.clone();
                            headers = tokio::task::block_in_place(|| {
                                h.block_on(async move {
                                    let mut result = HashMap::new();
                                    for (k, v) in headers_clone {
                                        let interpolated = interpolate_secrets_sync(&v, &db_clone2).await;
                                        result.insert(k, interpolated);
                                    }
                                    result
                                })
                            });

                            // Interpolate in body
                            if let Some(ref b) = body {
                                let body_clone = b.clone();
                                let db_clone3 = db.clone();
                                body = Some(tokio::task::block_in_place(|| {
                                    h.block_on(async move {
                                        interpolate_secrets_sync(&body_clone, &db_clone3).await
                                    })
                                }));
                            }
                        }
                    }

                    let options = HttpRequestOptions {
                        method: options_value.get("method")
                            .and_then(|v| v.as_str())
                            .unwrap_or("GET")
                            .to_string(),
                        url,
                        headers,
                        body,
                        timeout_secs: options_value.get("timeout")
                            .and_then(|v| v.as_u64()),
                        parse_json: options_value.get("parseJson")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false),
                    };

                    // Execute request
                    match http_request_sync(options, &config_clone3) {
                        Ok(response) => {
                            serde_json::json!({
                                "success": response.success,
                                "status": response.status,
                                "statusText": response.status_text,
                                "headers": response.headers,
                                "data": response.data,
                                "error": response.error
                            }).to_string()
                        }
                        Err(e) => {
                            serde_json::json!({
                                "success": false,
                                "status": 0,
                                "statusText": "Error",
                                "headers": {},
                                "data": null,
                                "error": format!("{}", e)
                            }).to_string()
                        }
                    }
                })?;
                ctx.globals().set("__rust_http_request", http_request_fn)?;

                Ok::<(), anyhow::Error>(())
            })?;
        }

        // Inject get_config function if database is available
        if let Some(ref db) = self.database {
            ctx.with(|ctx| {
                let database_clone = db.clone();
                let get_config_fn = Function::new(ctx.clone(), move |scope: String| -> String {
                    // Use blocking call to get config from database
                    if let Some(h) = tokio::runtime::Handle::try_current().ok() {
                        let db_clone = database_clone.clone();
                        tokio::task::block_in_place(|| {
                            h.block_on(async move {
                                match db_clone.get_plugin_config(&scope).await {
                                    Ok(Some(config)) => config,
                                    Ok(None) => {
                                        warn!("Config not found for scope: {}", scope);
                                        "{}".to_string()
                                    }
                                    Err(e) => {
                                        warn!("Failed to get config for scope {}: {}", scope, e);
                                        "{}".to_string()
                                    }
                                }
                            })
                        })
                    } else {
                        warn!("No tokio runtime available for get_config");
                        "{}".to_string()
                    }
                })?;
                ctx.globals().set("__rust_get_config", get_config_fn)?;
                Ok::<(), anyhow::Error>(())
            })?;
        }

        // Inject get_account_id function (read-only for tools)
        ctx.with(|ctx| {
            let account_id_clone = self.account_id.clone();
            let get_account_id_fn = Function::new(ctx.clone(), move || -> String {
                account_id_clone.clone()
            })?;
            ctx.globals().set("get_account_id", get_account_id_fn)?;

            // Inject get_current_session_id function (read-only for tools)
            let session_id_clone = self.session_id.clone();
            let get_current_session_id_fn = Function::new(ctx.clone(), move || -> String {
                session_id_clone.clone().unwrap_or_else(|| "".to_string())
            })?;
            ctx.globals().set("get_current_session_id", get_current_session_id_fn)?;

            // Add btoa (base64 encode) function with secret interpolation
            let database_clone_btoa = self.database.clone();
            let btoa_fn = Function::new(ctx.clone(), move |input: String| -> String {
                use base64::{Engine as _, engine::general_purpose};

                // Interpolate secrets before encoding
                let final_input = if let Some(ref db) = database_clone_btoa {
                    if let Some(h) = tokio::runtime::Handle::try_current().ok() {
                        let db_clone = db.clone();
                        let input_clone = input.clone();
                        tokio::task::block_in_place(|| {
                            h.block_on(async move {
                                interpolate_secrets_sync(&input_clone, &db_clone).await
                            })
                        })
                    } else {
                        input
                    }
                } else {
                    input
                };

                general_purpose::STANDARD.encode(final_input.as_bytes())
            })?;
            ctx.globals().set("btoa", btoa_fn)?;

            // Add atob (base64 decode) function
            let atob_fn = Function::new(ctx.clone(), |input: String| -> String {
                use base64::{Engine as _, engine::general_purpose};
                match general_purpose::STANDARD.decode(input.as_bytes()) {
                    Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
                    Err(_) => String::new()
                }
            })?;
            ctx.globals().set("atob", atob_fn)?;

            Ok::<(), anyhow::Error>(())
        })?;

        // Inject session functions if database is available
        if let Some(ref db) = self.database {
            ctx.with(|ctx| {
                let database_clone = db.clone();
                let account_id_clone = self.account_id.clone();

                // get_session(session_id) -> object
                let get_session_fn = Function::new(ctx.clone(), move |session_id: String| -> String {
                    let db_clone = database_clone.clone();
                    let account_id = account_id_clone.clone();

                    if let Some(h) = tokio::runtime::Handle::try_current().ok() {
                        tokio::task::block_in_place(|| {
                            h.block_on(async move {
                                match db_clone.get_session(&session_id, &account_id).await {
                                    Ok(Some((messages, metadata))) => {
                                        serde_json::json!({
                                            "messages": serde_json::from_str::<serde_json::Value>(&messages).unwrap_or(serde_json::json!([])),
                                            "metadata": serde_json::from_str::<serde_json::Value>(&metadata).unwrap_or(serde_json::json!({}))
                                        }).to_string()
                                    }
                                    Ok(None) => {
                                        serde_json::json!({}).to_string()
                                    }
                                    Err(e) => {
                                        serde_json::json!({
                                            "__error": format!("Failed to get session: {}", e)
                                        }).to_string()
                                    }
                                }
                            })
                        })
                    } else {
                        serde_json::json!({
                            "__error": "No tokio runtime available"
                        }).to_string()
                    }
                })?;
                ctx.globals().set("__rust_get_session", get_session_fn)?;

                let database_clone2 = db.clone();
                let account_id_clone2 = self.account_id.clone();

                // set_session(session_id, session_data) -> void
                let set_session_fn = Function::new(ctx.clone(), move |session_id: String, session_data_json: String| -> String {
                    let db_clone = database_clone2.clone();
                    let account_id = account_id_clone2.clone();

                    if let Some(h) = tokio::runtime::Handle::try_current().ok() {
                        tokio::task::block_in_place(|| {
                            h.block_on(async move {
                                // Parse session data
                                let session_data: serde_json::Value = match serde_json::from_str(&session_data_json) {
                                    Ok(v) => v,
                                    Err(e) => {
                                        return serde_json::json!({
                                            "__error": format!("Failed to parse session data: {}", e)
                                        }).to_string();
                                    }
                                };

                                // Extract messages and metadata
                                let messages = session_data.get("messages")
                                    .cloned()
                                    .unwrap_or(serde_json::json!([]));

                                let metadata = session_data.get("metadata")
                                    .cloned()
                                    .unwrap_or(serde_json::json!({}));

                                // Serialize to strings for database
                                let messages_str = messages.to_string();
                                let metadata_str = metadata.to_string();

                                match db_clone.upsert_session(&session_id, &account_id, &messages_str, &metadata_str).await {
                                    Ok(_) => serde_json::json!({ "success": true }).to_string(),
                                    Err(e) => {
                                        serde_json::json!({
                                            "__error": format!("Failed to save session: {}", e)
                                        }).to_string()
                                    }
                                }
                            })
                        })
                    } else {
                        serde_json::json!({
                            "__error": "No tokio runtime available"
                        }).to_string()
                    }
                })?;
                ctx.globals().set("__rust_set_session", set_session_fn)?;

                let database_clone3 = db.clone();
                let account_id_clone3 = self.account_id.clone();

                // clear_session(session_id) -> bool
                let clear_session_fn = Function::new(ctx.clone(), move |session_id: String| -> String {
                    let db_clone = database_clone3.clone();
                    let account_id = account_id_clone3.clone();

                    if let Some(h) = tokio::runtime::Handle::try_current().ok() {
                        tokio::task::block_in_place(|| {
                            h.block_on(async move {
                                match db_clone.delete_session(&session_id, &account_id).await {
                                    Ok(deleted) => serde_json::json!({
                                        "success": true,
                                        "deleted": deleted
                                    }).to_string(),
                                    Err(e) => {
                                        serde_json::json!({
                                            "__error": format!("Failed to clear session: {}", e)
                                        }).to_string()
                                    }
                                }
                            })
                        })
                    } else {
                        serde_json::json!({
                            "__error": "No tokio runtime available"
                        }).to_string()
                    }
                })?;
                ctx.globals().set("__rust_clear_session", clear_session_fn)?;

                Ok::<(), anyhow::Error>(())
            })?;
        }

        ctx.with(|ctx| {
            // Evaluate the tool code to get the tool object
            let tool_obj: Object = ctx.eval(js_code.as_bytes())?;

            // Get the execute function
            let execute_fn: rquickjs::Function = tool_obj.get("execute")?;

            // Inject _tool_name into args for multi-tool bundle support
            let mut args_with_tool_name = args.clone();
            if let Some(obj) = args_with_tool_name.as_object_mut() {
                obj.insert("_tool_name".to_string(), Value::String(tool_name.to_string()));
            } else {
                // If args is not an object, create one with _tool_name
                let mut new_obj = serde_json::Map::new();
                new_obj.insert("_tool_name".to_string(), Value::String(tool_name.to_string()));
                args_with_tool_name = Value::Object(new_obj);
            }

            // Convert args to JS object
            let args_str = serde_json::to_string(&args_with_tool_name)?;
            let args_js: JsValue = ctx.json_parse(args_str)?;

            // Call execute(args)
            let result: JsValue = execute_fn.call((args_js,))?;

            // Convert result back to JSON
            let result_str: String = result.as_string()
                .context("Tool execute() must return a string")?
                .to_string()?;

            let json_result: Value = serde_json::from_str(&result_str)
                .unwrap_or_else(|_| Value::String(result_str));

            Ok(json_result)
        })
    }

    /// Get all tools as OllamaFunction definitions for the LLM
    pub fn get_llm_tools(&self) -> Vec<OllamaFunction> {
        self.tools.values().map(|tool| {
            OllamaFunction {
                r#type: "function".to_string(),
                function: OllamaFunctionDefinition {
                    name: tool.name.clone(),
                    description: tool.description.clone(),
                    parameters: tool.schema.clone(),
                    required: tool.schema.required.clone().unwrap_or_default(),
                },
            }
        }).collect()
    }

    /// Get a list of all registered tool names
    pub fn list_tools(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }
}

// ============================================================================
// Schema Builder Helpers
// ============================================================================

/// Helper to build JSON schemas for tool parameters
pub struct SchemaBuilder {
    properties: BTreeMap<String, JsonSchema>,
    required: Vec<String>,
}

impl SchemaBuilder {
    pub fn new() -> Self {
        Self {
            properties: BTreeMap::new(),
            required: Vec::new(),
        }
    }

    /// Add a string parameter
    pub fn add_string(mut self, name: &str, description: &str, required: bool) -> Self {
        self.properties.insert(
            name.to_string(),
            JsonSchema {
                schema_type: "string".to_string(),
                description: Some(description.to_string()),
                properties: None,
                required: None,
            },
        );
        if required {
            self.required.push(name.to_string());
        }
        self
    }

    /// Add a number parameter
    pub fn add_number(mut self, name: &str, description: &str, required: bool) -> Self {
        self.properties.insert(
            name.to_string(),
            JsonSchema {
                schema_type: "number".to_string(),
                description: Some(description.to_string()),
                properties: None,
                required: None,
            },
        );
        if required {
            self.required.push(name.to_string());
        }
        self
    }

    /// Add a boolean parameter
    pub fn add_boolean(mut self, name: &str, description: &str, required: bool) -> Self {
        self.properties.insert(
            name.to_string(),
            JsonSchema {
                schema_type: "boolean".to_string(),
                description: Some(description.to_string()),
                properties: None,
                required: None,
            },
        );
        if required {
            self.required.push(name.to_string());
        }
        self
    }

    /// Build the final schema
    pub fn build(self) -> JsonSchema {
        JsonSchema {
            schema_type: "object".to_string(),
            description: None,
            properties: Some(self.properties),
            required: Some(self.required),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_tool_bundle_registration() {
        // Create a simple multi-tool bundle
        let js_code = r#"
            function register() {
                return [
                    {
                        name: "test_tool_1",
                        description: "First test tool",
                        parameters: {
                            type: "object",
                            properties: {
                                input: { type: "string", description: "Input" }
                            },
                            required: ["input"]
                        }
                    },
                    {
                        name: "test_tool_2",
                        description: "Second test tool",
                        parameters: {
                            type: "object",
                            properties: {
                                value: { type: "number", description: "Value" }
                            },
                            required: ["value"]
                        }
                    }
                ];
            }

            function execute(args) {
                const toolName = args && args._tool_name;
                if (toolName === "test_tool_1") {
                    return JSON.stringify({ success: true, tool: "test_tool_1", input: args.input });
                }
                if (toolName === "test_tool_2") {
                    return JSON.stringify({ success: true, tool: "test_tool_2", value: args.value });
                }
                return JSON.stringify({ success: false, error: "Unknown tool" });
            }

            ({ register, execute })
        "#;

        // Create a tool registry
        let config = JsRuntimeConfig::default();
        let mut registry = ToolRegistry::new(config, None, None, None, None).unwrap();

        // Register the multi-tool bundle
        registry.register_tool_from_js(js_code).unwrap();

        // Verify both tools were registered
        assert!(registry.has_tool("test_tool_1"), "test_tool_1 should be registered");
        assert!(registry.has_tool("test_tool_2"), "test_tool_2 should be registered");

        // Test executing test_tool_1
        let args1 = serde_json::json!({ "input": "hello" });
        let result1 = registry.execute("test_tool_1", args1).unwrap();
        assert_eq!(result1["success"], true);
        assert_eq!(result1["tool"], "test_tool_1");
        assert_eq!(result1["input"], "hello");

        // Test executing test_tool_2
        let args2 = serde_json::json!({ "value": 42 });
        let result2 = registry.execute("test_tool_2", args2).unwrap();
        assert_eq!(result2["success"], true);
        assert_eq!(result2["tool"], "test_tool_2");
        assert_eq!(result2["value"], 42);
    }

    #[test]
    fn test_single_tool_registration() {
        // Create a simple single tool
        let js_code = r#"
            function register() {
                return {
                    name: "single_test_tool",
                    description: "A single test tool",
                    parameters: {
                        type: "object",
                        properties: {
                            message: { type: "string", description: "Message" }
                        },
                        required: ["message"]
                    }
                };
            }

            function execute(args) {
                return JSON.stringify({ success: true, message: args.message });
            }

            ({ register, execute })
        "#;

        // Create a tool registry
        let config = JsRuntimeConfig::default();
        let mut registry = ToolRegistry::new(config, None, None, None, None).unwrap();

        // Register the single tool
        registry.register_tool_from_js(js_code).unwrap();

        // Verify the tool was registered
        assert!(registry.has_tool("single_test_tool"), "single_test_tool should be registered");

        // Test executing the tool
        let args = serde_json::json!({ "message": "test message" });
        let result = registry.execute("single_test_tool", args).unwrap();
        assert_eq!(result["success"], true);
        assert_eq!(result["message"], "test message");
    }
}

