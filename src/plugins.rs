use anyhow::Result;
use rquickjs::{Context, Runtime, Function};
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::rc::Rc;
use tracing::{info, warn};

use crate::js_runtime::{JsRuntimeConfig, setup_js_globals, http_get_sync, http_post_sync, http_request_sync, HttpRequestOptions};

#[derive(Debug, Clone)]
pub struct Plugin {
    pub name: String,
    pub description: String,
    pub priority: u32,
    pub hooks: HashSet<String>,
    js_code: String,
}

pub struct PluginRegistry {
    plugins: Vec<Plugin>,
    runtime: Rc<Runtime>,
    config: JsRuntimeConfig,
    enabled_plugins: Option<Vec<String>>,
    disabled_plugins: Option<Vec<String>>,
}

impl PluginRegistry {
    pub fn new(config: JsRuntimeConfig, enabled_plugins: Option<Vec<String>>, disabled_plugins: Option<Vec<String>>) -> Result<Self> {
        let runtime = Runtime::new()?;

        // Setup globals in the runtime
        let ctx = Context::full(&runtime)?;
        setup_js_globals(&ctx, &config)?;

        Ok(Self {
            plugins: Vec::new(),
            runtime: Rc::new(runtime),
            config,
            enabled_plugins,
            disabled_plugins,
        })
    }

    /// Check if a plugin should be loaded based on enabled/disabled lists
    fn should_load_plugin(&self, plugin_name: &str) -> bool {
        // If enabled_plugins is specified, only load plugins in that list
        if let Some(ref enabled) = self.enabled_plugins {
            return enabled.contains(&plugin_name.to_string());
        }

        // If disabled_plugins is specified, don't load plugins in that list
        if let Some(ref disabled) = self.disabled_plugins {
            return !disabled.contains(&plugin_name.to_string());
        }

        // By default, load all plugins
        true
    }

    /// Load all plugins from a directory
    pub fn load_plugins_from_dir<P: AsRef<Path>>(&mut self, dir: P) -> Result<()> {
        let dir_path = dir.as_ref();
        if !dir_path.exists() {
            warn!("Plugins directory does not exist: {:?}", dir_path);
            return Ok(());
        }

        for entry in fs::read_dir(dir_path)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("js") {
                let js_code = fs::read_to_string(&path)?;
                match self.register_plugin_from_js(&js_code) {
                    Ok(true) => {
                        info!("✓ Loaded plugin: {}", path.file_name().unwrap().to_string_lossy());
                    }
                    Ok(false) => {
                        // Plugin was filtered out, don't log
                    }
                    Err(e) => {
                        warn!("✗ Failed to load plugin {}: {}", path.display(), e);
                    }
                }
            }
        }

        // Sort plugins by priority (lower priority runs first)
        self.plugins.sort_by_key(|p| p.priority);

        Ok(())
    }

    /// Register a plugin from JavaScript code
    /// Returns Ok(true) if plugin was registered, Ok(false) if filtered out
    fn register_plugin_from_js(&mut self, js_code: &str) -> Result<bool> {
        let ctx = Context::full(&self.runtime)?;

        ctx.with(|ctx| {
            // Evaluate the plugin code
            let wrapper = format!(
                r#"
                {}
                register();
                "#,
                js_code
            );

            let metadata: rquickjs::Value = ctx.eval(wrapper.as_bytes())?;

            // Extract plugin metadata
            let obj = metadata.as_object().ok_or_else(|| anyhow::anyhow!("register() must return an object"))?;

            let name: String = obj.get("name")?;

            // Check if this plugin should be loaded
            if !self.should_load_plugin(&name) {
                return Ok(false);
            }

            let description: String = obj.get("description")?;
            let priority: u32 = obj.get::<_, Option<u32>>("priority")?.unwrap_or(50);
            
            // Extract hooks
            let hooks_obj: rquickjs::Object = obj.get("hooks")?;
            let mut hooks = HashSet::new();
            
            for hook_name in ["pre_request", "post_response", "on_stream_chunk",
                               "on_conversation_turn", "on_tool_call", "on_tool_result"] {
                if hooks_obj.get::<_, Option<bool>>(hook_name)?.unwrap_or(false) {
                    hooks.insert(hook_name.to_string());
                }
            }
            
            let plugin = Plugin {
                name,
                description,
                priority,
                hooks,
                js_code: js_code.to_string(),
            };
            
            self.plugins.push(plugin);

            Ok::<bool, anyhow::Error>(true)
        })
    }

    /// Execute a hook with the given data
    /// Returns the modified data after all plugins have processed it
    pub fn execute_hook(&self, hook_name: &str, mut data: Value) -> Result<Value> {
        for plugin in &self.plugins {
            if plugin.hooks.contains(hook_name) {
                match self.execute_plugin_hook(&plugin, hook_name, data.clone()) {
                    Ok(modified_data) => {
                        data = modified_data;
                    }
                    Err(e) => {
                        eprintln!("Error executing plugin '{}' hook '{}': {}", plugin.name, hook_name, e);
                        // Continue with unmodified data
                    }
                }
            }
        }
        Ok(data)
    }

    /// Execute a specific plugin's hook handler
    fn execute_plugin_hook(&self, plugin: &Plugin, hook_name: &str, data: Value) -> Result<Value> {
        let ctx = Context::full(&self.runtime)?;

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

            // Load the plugin code
            let _: rquickjs::Value = ctx.eval(plugin.js_code.as_bytes())
                .map_err(|e| anyhow::anyhow!("Failed to load plugin code: {:?}", e))?;

            // Get the hook function
            let global = ctx.globals();
            let hook_fn: Function = global.get(hook_name)
                .map_err(|e| anyhow::anyhow!("Failed to get hook function '{}': {:?}", hook_name, e))?;

            // Convert data to JS value
            let data_str = serde_json::to_string(&data)?;
            let js_data: rquickjs::Value = ctx.json_parse(data_str)
                .map_err(|e| anyhow::anyhow!("Failed to parse data to JS: {:?}", e))?;

            // Call the hook function
            let result: rquickjs::Value = hook_fn.call((js_data,))
                .map_err(|e| anyhow::anyhow!("Failed to call hook function: {:?}", e))?;

            // Convert result back to JSON
            let result_str_opt = ctx.json_stringify(result)?;
            let result_str = if let Some(s) = result_str_opt {
                s.to_string()?
            } else {
                "null".to_string()
            };
            let result_value: Value = serde_json::from_str(&result_str)?;

            Ok(result_value)
        })
    }

    pub fn get_plugins(&self) -> &[Plugin] {
        &self.plugins
    }
}

