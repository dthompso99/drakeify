use anyhow::{Context, Result};
use rquickjs::{Context as JsContext, Runtime, Value as JsValue, Object, Function};
use serde_json::Value;
use std::collections::{HashMap, BTreeMap};
use std::fs;
use std::path::Path;

use crate::llm::{JsonSchema, OllamaFunction, OllamaFunctionDefinition};
use crate::js_runtime::{JsRuntimeConfig, setup_js_globals, http_get_sync, http_post_sync, http_request_sync, HttpRequestOptions};

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
}

impl ToolRegistry {
    /// Create a new tool registry with a QuickJS runtime
    pub fn new(config: JsRuntimeConfig, enabled_tools: Option<Vec<String>>, disabled_tools: Option<Vec<String>>) -> Result<Self> {
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
        })
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

            // Only process .js files
            if path.extension().and_then(|s| s.to_str()) == Some("js") {
                let js_code = fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read tool file: {:?}", path))?;

                self.register_tool_from_js(&js_code)
                    .with_context(|| format!("Failed to register tool from: {:?}", path))?;
            }
        }

        Ok(())
    }

    /// Register a tool by calling its register() function
    fn register_tool_from_js(&mut self, js_code: &str) -> Result<()> {
        let ctx = JsContext::full(&self.js_runtime)?;

        ctx.with(|ctx| {
            // Evaluate the tool code
            let tool_obj: Object = ctx.eval(js_code.as_bytes())?;

            // Call the register() function
            let register_fn: rquickjs::Function = tool_obj.get("register")?;
            let metadata: Object = register_fn.call(())?;

            // Extract metadata
            let name: String = metadata.get("name")?;

            // Check if this tool should be loaded
            if !self.should_load_tool(&name) {
                return Ok(());
            }

            let description: String = metadata.get("description")?;
            let params_obj: Object = metadata.get("parameters")?;

            // Convert parameters to JsonSchema
            let schema = self.parse_schema_from_js(&ctx, params_obj)?;

            // Create and register the tool
            let tool = Tool {
                name: name.clone(),
                description,
                schema,
                js_code: js_code.to_string(),
            };

            self.tools.insert(name, tool);
            Ok(())
        })
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

        self.execute_js_tool(&tool.js_code, args)
    }

    /// Execute a JavaScript tool by calling its execute() function
    fn execute_js_tool(&self, js_code: &str, args: Value) -> Result<Value> {
        let ctx = JsContext::full(&self.js_runtime)?;

        // Setup globals for this execution context
        setup_js_globals(&ctx, &self.config)?;

        // Inject HTTP functions if allowed
        if self.config.allow_http {
            ctx.with(|ctx| {
                // Legacy GET function
                let config_clone = self.config.clone();
                let http_get_fn = Function::new(ctx.clone(), move |url: String| -> String {
                    match http_get_sync(url, &config_clone) {
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
                let http_post_fn = Function::new(ctx.clone(), move |url: String, body: String| -> String {
                    match http_post_sync(url, body, &config_clone2) {
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
                    let options = HttpRequestOptions {
                        method: options_value.get("method")
                            .and_then(|v| v.as_str())
                            .unwrap_or("GET")
                            .to_string(),
                        url: options_value.get("url")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        headers: options_value.get("headers")
                            .and_then(|v| v.as_object())
                            .map(|obj| {
                                obj.iter()
                                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                                    .collect()
                            })
                            .unwrap_or_default(),
                        body: options_value.get("body")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
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

        ctx.with(|ctx| {
            // Evaluate the tool code to get the tool object
            let tool_obj: Object = ctx.eval(js_code.as_bytes())?;

            // Get the execute function
            let execute_fn: rquickjs::Function = tool_obj.get("execute")?;

            // Convert args to JS object
            let args_str = serde_json::to_string(&args)?;
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

