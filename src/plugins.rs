use anyhow::Result;
use rquickjs::{Context, Runtime, Function};
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::rc::Rc;
use std::cell::RefCell;
use tracing::{info, warn};

use crate::js_runtime::{JsRuntimeConfig, setup_js_globals, http_get_sync, http_post_sync, http_request_sync, HttpRequestOptions};
use crate::database::Database;
use crate::llm::{LlmConfig, OllamaRequest, OllamaOptions, OllamaMessage};
use crate::session::SessionMetadata;
use std::collections::HashMap;

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
    database: Option<std::sync::Arc<Database>>,
    account_id: Rc<RefCell<String>>,
    llm_config: Option<LlmConfig>,
    llm_model: Option<String>,
}

impl PluginRegistry {
    pub fn new(
        config: JsRuntimeConfig,
        enabled_plugins: Option<Vec<String>>,
        disabled_plugins: Option<Vec<String>>,
        database: Option<std::sync::Arc<Database>>,
        account_id: Option<String>,
        llm_config: Option<LlmConfig>,
        llm_model: Option<String>,
    ) -> Result<Self> {
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
            database,
            account_id: Rc::new(RefCell::new(account_id.unwrap_or_else(|| "anonymous".to_string()))),
            llm_config,
            llm_model,
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

            // Check if it's a .js file directly in the directory
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("js") {
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
            // Check if it's a directory with a plugin.js file (installed package format)
            else if path.is_dir() {
                let plugin_file = path.join("plugin.js");
                if plugin_file.exists() {
                    let js_code = fs::read_to_string(&plugin_file)?;
                    match self.register_plugin_from_js(&js_code) {
                        Ok(true) => {
                            info!("✓ Loaded plugin: {}", path.file_name().unwrap().to_string_lossy());
                        }
                        Ok(false) => {
                            // Plugin was filtered out, don't log
                        }
                        Err(e) => {
                            warn!("✗ Failed to load plugin {}: {}", plugin_file.display(), e);
                        }
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
                               "on_conversation_turn", "on_tool_call", "on_tool_result", "on_webhook_call"] {
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

    /// Get all plugins that have a specific hook
    pub fn get_plugins_with_hook(&self, hook_name: &str) -> Vec<&Plugin> {
        self.plugins.iter()
            .filter(|p| p.hooks.contains(hook_name))
            .collect()
    }

    /// Execute a hook with the given data
    /// Returns the modified data after all plugins have processed it
    pub fn execute_hook(&self, hook_name: &str, mut data: Value) -> Result<Value> {
        let plugins_with_hook: Vec<&Plugin> = self.plugins.iter()
            .filter(|p| p.hooks.contains(hook_name))
            .collect();

        if !plugins_with_hook.is_empty() {
            tracing::debug!("Executing {} hook for {} plugin(s)", hook_name, plugins_with_hook.len());
        }

        for plugin in plugins_with_hook {
            tracing::debug!("  → Executing {} hook for plugin: {}", hook_name, plugin.name);
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
        Ok(data)
    }

    /// Execute a webhook hook for a specific plugin by name
    pub fn execute_webhook_hook(&self, plugin_name: &str, data: Value) -> Result<Value> {
        let plugin = self.plugins.iter()
            .find(|p| p.name == plugin_name)
            .ok_or_else(|| anyhow::anyhow!("Plugin '{}' not found", plugin_name))?;

        if !plugin.hooks.contains("on_webhook_call") {
            return Err(anyhow::anyhow!("Plugin '{}' does not have on_webhook_call hook", plugin_name));
        }

        self.execute_plugin_hook(plugin, "on_webhook_call", data)
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
                let database_clone = self.database.clone();
                let http_get_fn = Function::new(ctx.clone(), move |url: String| -> String {
                    let mut final_url = url;

                    // Interpolate secrets if database is available
                    if let Some(ref db) = database_clone {
                        if let Some(h) = tokio::runtime::Handle::try_current().ok() {
                            let db_clone = db.clone();
                            let url_clone = final_url.clone();
                            eprintln!("[DEBUG] [Plugin HTTP GET] BEFORE interpolation: {}", url_clone);
                            final_url = tokio::task::block_in_place(|| {
                                h.block_on(async move {
                                    interpolate_secrets_sync(&url_clone, &db_clone).await
                                })
                            });
                            eprintln!("[DEBUG] [Plugin HTTP GET] AFTER interpolation: {}", final_url);
                        }
                    }

                    eprintln!("[DEBUG] [Plugin HTTP GET] Final URL being sent: {}", final_url);
                    match http_get_sync(final_url, &config_clone) {
                        Ok(data) => data,
                        Err(e) => {
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
                            // Interpolate URL
                            let db_clone = db.clone();
                            let url_clone = final_url.clone();
                            eprintln!("[DEBUG] [Plugin HTTP POST] BEFORE URL interpolation: {}", url_clone);
                            final_url = tokio::task::block_in_place(|| {
                                h.block_on(async move {
                                    interpolate_secrets_sync(&url_clone, &db_clone).await
                                })
                            });
                            eprintln!("[DEBUG] [Plugin HTTP POST] AFTER URL interpolation: {}", final_url);

                            // Interpolate body
                            let db_clone2 = db.clone();
                            let body_clone = final_body.clone();
                            eprintln!("[DEBUG] [Plugin HTTP POST] BEFORE body interpolation: {}", body_clone);
                            final_body = tokio::task::block_in_place(|| {
                                h.block_on(async move {
                                    interpolate_secrets_sync(&body_clone, &db_clone2).await
                                })
                            });
                            eprintln!("[DEBUG] [Plugin HTTP POST] AFTER body interpolation: {}", final_body);
                        }
                    }

                    eprintln!("[DEBUG] [Plugin HTTP POST] Final URL: {}", final_url);
                    eprintln!("[DEBUG] [Plugin HTTP POST] Final body: {}", final_body);
                    match http_post_sync(final_url, final_body, &config_clone2) {
                        Ok(data) => data,
                        Err(e) => {
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
                        if let Some(h) = tokio::runtime::Handle::try_current().ok() {
                            // Interpolate URL
                            let db_clone = db.clone();
                            let url_clone = url.clone();
                            eprintln!("[DEBUG] [Plugin HTTP REQUEST] BEFORE URL interpolation: {}", url_clone);
                            url = tokio::task::block_in_place(|| {
                                h.block_on(async move {
                                    interpolate_secrets_sync(&url_clone, &db_clone).await
                                })
                            });
                            eprintln!("[DEBUG] [Plugin HTTP REQUEST] AFTER URL interpolation: {}", url);

                            // Interpolate in headers
                            let headers_clone = headers.clone();
                            let db_clone2 = db.clone();
                            eprintln!("[DEBUG] [Plugin HTTP REQUEST] BEFORE headers interpolation: {:?}", headers_clone);
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
                            eprintln!("[DEBUG] [Plugin HTTP REQUEST] AFTER headers interpolation: {:?}", headers);

                            // Interpolate in body
                            if let Some(ref b) = body {
                                let body_clone = b.clone();
                                let db_clone3 = db.clone();
                                eprintln!("[DEBUG] [Plugin HTTP REQUEST] BEFORE body interpolation: {}", body_clone);
                                body = Some(tokio::task::block_in_place(|| {
                                    h.block_on(async move {
                                        interpolate_secrets_sync(&body_clone, &db_clone3).await
                                    })
                                }));
                                eprintln!("[DEBUG] [Plugin HTTP REQUEST] AFTER body interpolation: {:?}", body);
                            }
                        }
                    }

                    eprintln!("[DEBUG] [Plugin HTTP REQUEST] Final URL: {}", url);
                    eprintln!("[DEBUG] [Plugin HTTP REQUEST] Final headers: {:?}", headers);
                    eprintln!("[DEBUG] [Plugin HTTP REQUEST] Final body: {:?}", body);

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
                    let db_clone = database_clone.clone();
                    let scope_clone = scope.clone();

                    // Use a channel to get the result from the async task
                    let (tx, rx) = std::sync::mpsc::channel();

                    if let Some(h) = tokio::runtime::Handle::try_current().ok() {
                        h.spawn(async move {
                            let result = async move {
                                match db_clone.get_plugin_config(&scope_clone).await {
                                    Ok(Some(config)) => config,
                                    Ok(None) => {
                                        warn!("Config not found for scope: {}", scope_clone);
                                        "{}".to_string()
                                    }
                                    Err(e) => {
                                        warn!("Failed to get config for scope {}: {}", scope_clone, e);
                                        "{}".to_string()
                                    }
                                }
                            }.await;
                            let _ = tx.send(result);
                        });

                        // Wait for the result with a timeout
                        rx.recv_timeout(std::time::Duration::from_secs(10))
                            .unwrap_or_else(|_| {
                                warn!("Config fetch timed out for scope: {}", scope);
                                "{}".to_string()
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

        // Inject account_id functions
        ctx.with(|ctx| {
            let account_id_clone = self.account_id.clone();
            let get_account_id_fn = Function::new(ctx.clone(), move || -> String {
                account_id_clone.borrow().clone()
            })?;
            ctx.globals().set("get_account_id", get_account_id_fn)?;

            let account_id_clone2 = self.account_id.clone();
            let set_account_id_fn = Function::new(ctx.clone(), move |new_id: String| {
                *account_id_clone2.borrow_mut() = new_id;
            })?;
            ctx.globals().set("set_account_id", set_account_id_fn)?;

            // Add btoa (base64 encode) function
            let btoa_fn = Function::new(ctx.clone(), |input: String| -> String {
                use base64::{Engine as _, engine::general_purpose};
                general_purpose::STANDARD.encode(input.as_bytes())
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

                // get_session(session_id) -> object | null
                let get_session_fn = Function::new(ctx.clone(), move |session_id: String| -> String {
                    let db_clone = database_clone.clone();
                    let account_id = account_id_clone.borrow().clone();

                    // Use a channel to get the result from the async task
                    let (tx, rx) = std::sync::mpsc::channel();

                    if let Some(h) = tokio::runtime::Handle::try_current().ok() {
                        h.spawn(async move {
                            let result = async move {
                                match db_clone.get_session(&session_id, &account_id).await {
                                    Ok(Some((messages_json, metadata_json))) => {
                                        // Parse the JSON strings from the database
                                        let messages: serde_json::Value = match serde_json::from_str(&messages_json) {
                                            Ok(v) => v,
                                            Err(e) => {
                                                return serde_json::json!({
                                                    "__error": format!("Failed to parse messages JSON: {}", e)
                                                }).to_string();
                                            }
                                        };

                                        let metadata: serde_json::Value = match serde_json::from_str(&metadata_json) {
                                            Ok(v) => v,
                                            Err(e) => {
                                                return serde_json::json!({
                                                    "__error": format!("Failed to parse metadata JSON: {}", e)
                                                }).to_string();
                                            }
                                        };

                                        // Return session object with parsed messages and metadata
                                        serde_json::json!({
                                            "messages": messages,
                                            "metadata": metadata
                                        }).to_string()
                                    }
                                    Ok(None) => "null".to_string(),
                                    Err(e) => {
                                        // Throw error by returning error JSON
                                        serde_json::json!({
                                            "__error": format!("Failed to get session: {}", e)
                                        }).to_string()
                                    }
                                }
                            }.await;
                            let _ = tx.send(result);
                        });

                        // Wait for the result with a timeout
                        rx.recv_timeout(std::time::Duration::from_secs(10))
                            .unwrap_or_else(|_| serde_json::json!({
                                "__error": "Session fetch timed out"
                            }).to_string())
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
                    let account_id = account_id_clone2.borrow().clone();

                    // Use a channel to get the result from the async task
                    let (tx, rx) = std::sync::mpsc::channel();

                    if let Some(h) = tokio::runtime::Handle::try_current().ok() {
                        h.spawn(async move {
                            let result = async move {
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
                                    .and_then(|v| serde_json::from_value::<SessionMetadata>(v.clone()).ok())
                                    .unwrap_or_default();

                                // Serialize to strings for database
                                let messages_str = messages.to_string();
                                let metadata_str = serde_json::to_string(&metadata).unwrap_or_else(|_| "{}".to_string());

                                match db_clone.upsert_session(&session_id, &account_id, &messages_str, &metadata_str).await {
                                    Ok(_) => serde_json::json!({ "success": true }).to_string(),
                                    Err(e) => {
                                        serde_json::json!({
                                            "__error": format!("Failed to save session: {}", e)
                                        }).to_string()
                                    }
                                }
                            }.await;
                            let _ = tx.send(result);
                        });

                        // Wait for the result with a timeout
                        rx.recv_timeout(std::time::Duration::from_secs(10))
                            .unwrap_or_else(|_| serde_json::json!({
                                "__error": "Session save timed out"
                            }).to_string())
                    } else {
                        serde_json::json!({
                            "__error": "No tokio runtime available"
                        }).to_string()
                    }
                })?;
                ctx.globals().set("__rust_set_session", set_session_fn)?;

                Ok::<(), anyhow::Error>(())
            })?;
        }

        // Inject call_llm function if llm_config is available
        if let (Some(llm_config), Some(llm_model)) = (&self.llm_config, &self.llm_model) {
            ctx.with(|ctx| {
                let llm_config_clone = llm_config.clone();
                let llm_model_clone = llm_model.clone();

                // call_llm(options) -> object
                let call_llm_fn = Function::new(ctx.clone(), move |options_json: String| -> String {
                    tokio::task::block_in_place(|| {
                        if let Some(h) = tokio::runtime::Handle::try_current().ok() {
                            let config = llm_config_clone.clone();
                            let default_model = llm_model_clone.clone();

                            h.block_on(async {
                                // Parse options
                                let options: serde_json::Value = match serde_json::from_str(&options_json) {
                                    Ok(v) => v,
                                    Err(e) => {
                                        return serde_json::json!({
                                            "__error": format!("Failed to parse options: {}", e)
                                        }).to_string();
                                    }
                                };

                                // Extract messages
                                let messages_value = match options.get("messages") {
                                    Some(v) => v.clone(),
                                    None => {
                                        return serde_json::json!({
                                            "__error": "Missing required field: messages"
                                        }).to_string();
                                    }
                                };

                                let messages: Vec<OllamaMessage> = match serde_json::from_value(messages_value) {
                                    Ok(m) => m,
                                    Err(e) => {
                                        return serde_json::json!({
                                            "__error": format!("Failed to parse messages: {}", e)
                                        }).to_string();
                                    }
                                };

                                // Extract optional model (default to config model)
                                let model = options.get("model")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or(&default_model)
                                    .to_string();

                                // Build LLM request
                                let request = OllamaRequest {
                                    model,
                                    prompt: None,
                                    stream: false,
                                    think: false,
                                    options: OllamaOptions {
                                        num_ctx: 8192, // Default context size
                                    },
                                    messages,
                                    tools: vec![], // No tool support for now
                                    tool_choice: None,
                                };

                                // Execute LLM request
                                match crate::llm::execute_request(request, &config, true, None).await {
                                    Ok(response) => {
                                        serde_json::json!({
                                            "content": response.content,
                                            "tool_calls": response.tool_calls
                                        }).to_string()
                                    }
                                    Err(e) => {
                                        serde_json::json!({
                                            "__error": format!("LLM request failed: {}", e)
                                        }).to_string()
                                    }
                                }
                            })
                        } else {
                            serde_json::json!({
                                "__error": "No tokio runtime available"
                            }).to_string()
                        }
                    })
                })?;
                ctx.globals().set("__rust_call_llm", call_llm_fn)?;

                Ok::<(), anyhow::Error>(())
            })?;
        }

        // Inject process_conversation function if llm_config is available
        if let (Some(llm_config), Some(llm_model)) = (&self.llm_config, &self.llm_model) {
            ctx.with(|ctx| {
                let llm_config_clone = llm_config.clone();
                let llm_model_clone = llm_model.clone();
                let js_config_clone = self.config.clone();
                let enabled_plugins_clone = self.enabled_plugins.clone();
                let disabled_plugins_clone = self.disabled_plugins.clone();
                let database_clone = self.database.clone();
                let account_id_rc = self.account_id.clone(); // Clone the Rc<RefCell>, not the inner value

                // process_conversation(messages) -> object
                let process_conversation_fn = Function::new(ctx.clone(), move |messages_json: String| -> String {
                    tokio::task::block_in_place(|| {
                        if let Some(h) = tokio::runtime::Handle::try_current().ok() {
                            let config = llm_config_clone.clone();
                            let model = llm_model_clone.clone();
                            let js_config = js_config_clone.clone();
                            let enabled_plugins = enabled_plugins_clone.clone();
                            let disabled_plugins = disabled_plugins_clone.clone();
                            let database = database_clone.clone();
                            let account_id = account_id_rc.borrow().clone(); // Read current value at call time

                            h.block_on(async {
                                // Parse messages
                                let messages: Vec<OllamaMessage> = match serde_json::from_str(&messages_json) {
                                    Ok(m) => m,
                                    Err(e) => {
                                        return serde_json::json!({
                                            "__error": format!("Failed to parse messages: {}", e)
                                        }).to_string();
                                    }
                                };

                                // Create tool registry
                                let mut tool_registry = match crate::tools::ToolRegistry::new(
                                    js_config.clone(),
                                    None, // enabled_tools - use all tools
                                    None, // disabled_tools
                                    database.clone(),
                                    Some(account_id.clone())
                                ) {
                                    Ok(r) => r,
                                    Err(e) => {
                                        return serde_json::json!({
                                            "__error": format!("Failed to create tool registry: {}", e)
                                        }).to_string();
                                    }
                                };

                                // Load tools from directory
                                if let Err(e) = tool_registry.load_tools_from_dir("data/tools") {
                                    return serde_json::json!({
                                        "__error": format!("Failed to load tools: {}", e)
                                    }).to_string();
                                }

                                // Debug: Log loaded tools
                                let tool_names = tool_registry.list_tools();
                                eprintln!("[process_conversation] Loaded {} tools: {:?}", tool_names.len(), tool_names);

                                // Create plugin registry
                                let mut plugin_registry = match crate::plugins::PluginRegistry::new(
                                    js_config,
                                    enabled_plugins,
                                    disabled_plugins,
                                    database,
                                    Some(account_id),
                                    Some(config.clone()),
                                    Some(model.clone())
                                ) {
                                    Ok(r) => r,
                                    Err(e) => {
                                        return serde_json::json!({
                                            "__error": format!("Failed to create plugin registry: {}", e)
                                        }).to_string();
                                    }
                                };

                                // Load plugins from directory
                                if let Err(e) = plugin_registry.load_plugins_from_dir("data/plugins") {
                                    return serde_json::json!({
                                        "__error": format!("Failed to load plugins: {}", e)
                                    }).to_string();
                                }

                                // Execute conversation loop
                                match crate::execute_conversation_loop(
                                    messages,
                                    &config,
                                    &model,
                                    8192, // Default context size
                                    &tool_registry,
                                    &plugin_registry
                                ).await {
                                    Ok(response) => {
                                        serde_json::json!({
                                            "content": response
                                        }).to_string()
                                    }
                                    Err(e) => {
                                        serde_json::json!({
                                            "__error": format!("Conversation loop failed: {}", e)
                                        }).to_string()
                                    }
                                }
                            })
                        } else {
                            serde_json::json!({
                                "__error": "No tokio runtime available"
                            }).to_string()
                        }
                    })
                })?;
                ctx.globals().set("__rust_process_conversation", process_conversation_fn)?;

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

