use anyhow::{Context, Result};
use axum::{
    extract::{ws::WebSocketUpgrade, Path, Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use drakeify::database::{Database, LlmConfigRecord};
use drakeify::registry::{PackageMetadata, PackageType, RegistryClient};

/// Shared application state
#[derive(Clone)]
struct AppState {
    db: Arc<Database>,
    auth_token: String,
    // Broadcast channel for live updates
    update_tx: broadcast::Sender<ConfigUpdate>,
    // Plugin installation directory
    plugin_dir: std::path::PathBuf,
    // Tool installation directory
    tool_dir: std::path::PathBuf,
    // Registry URL for plugin/tool discovery
    registry_url: String,
}

/// Configuration update event for WebSocket broadcasting
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
enum ConfigUpdate {
    LlmConfigCreated { id: String },
    LlmConfigUpdated { id: String },
    LlmConfigDeleted { id: String },
    PluginInstalled { name: String },
    PluginUninstalled { name: String },
    ToolInstalled { name: String },
    ToolUninstalled { name: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("🌐 Drakeify Web UI starting up");

    // Load configuration
    let database_url = std::env::var("DATABASE_URL")
        .context("DATABASE_URL environment variable not set")?;

    let auth_token = std::env::var("DRAKEIFY_WEB_TOKEN")
        .context("DRAKEIFY_WEB_TOKEN environment variable not set")?;

    // Plugin directory (shared volume)
    let plugin_dir = std::path::PathBuf::from(
        std::env::var("PLUGIN_DIR").unwrap_or_else(|_| "/data/plugins".to_string())
    );

    // Tool directory (shared volume)
    let tool_dir = std::path::PathBuf::from(
        std::env::var("TOOL_DIR").unwrap_or_else(|_| "/data/tools".to_string())
    );

    // Registry URL for plugin/tool discovery
    let registry_url = std::env::var("DRAKEIFY_REGISTRY_URL")
        .unwrap_or_else(|_| "https://zot.hallrd.click".to_string());

    // Connect to database
    let db = Database::connect(&database_url).await?;
    info!("✓ Connected to database");

    // Run migrations
    db.migrate().await?;
    info!("✓ Database migrations complete");

    // Create broadcast channel for live updates
    let (update_tx, _) = broadcast::channel(100);

    // Build application state
    let state = AppState {
        db: Arc::new(db),
        auth_token,
        update_tx,
        plugin_dir,
        tool_dir,
        registry_url,
    };

    // Build API router with authentication
    let api_router = Router::new()
        // LLM Config endpoints
        .route("/llm/configs", get(list_llm_configs))
        .route("/llm/configs", post(create_llm_config))
        .route("/llm/configs/:id", get(get_llm_config))
        .route("/llm/configs/:id", put(update_llm_config))
        .route("/llm/configs/:id", delete(delete_llm_config))
        // Plugin endpoints
        .route("/plugins", get(list_plugins))
        .route("/plugins/available", get(list_available_plugins))
        .route("/plugins/unpublished", get(list_unpublished_plugins))
        .route("/plugins/:name/tags", get(get_plugin_tags))
        .route("/plugins/:name/metadata", get(get_plugin_metadata))
        .route("/plugins/:name/config", get(get_plugin_config))
        .route("/plugins/:name/config", put(update_plugin_config))
        .route("/plugins/:name/config", delete(delete_plugin_config))
        .route("/plugins/install", post(install_plugin))
        .route("/plugins/publish", post(publish_plugin))
        .route("/plugins/:name", delete(uninstall_plugin))
        // Tool endpoints
        .route("/tools", get(list_tools))
        .route("/tools/available", get(list_available_tools))
        .route("/tools/unpublished", get(list_unpublished_tools))
        .route("/tools/:name/tags", get(get_tool_tags))
        .route("/tools/:name/metadata", get(get_tool_metadata))
        .route("/tools/:name/config", get(get_tool_config))
        .route("/tools/:name/config", put(update_tool_config))
        .route("/tools/:name/config", delete(delete_tool_config))
        .route("/tools/install", post(install_tool))
        .route("/tools/publish", post(publish_tool))
        .route("/tools/:name", delete(uninstall_tool))
        // Session endpoints
        .route("/sessions", get(list_sessions))
        .route("/sessions/:session_id", get(get_session))
        // Secrets endpoints
        .route("/secrets/:key", get(get_secret))
        .route("/secrets/:key", put(set_secret))
        .route("/secrets/:key", delete(delete_secret))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware));

    // Build WebSocket router (auth handled in websocket_handler via query param)
    let ws_router = Router::new()
        .route("/ws", get(websocket_handler))
        .with_state(state.clone());

    // Build main router
    let app = Router::new()
        // Static pages (no auth required)
        .route("/", get(index_page))
        .route("/health", get(health_check))
        .route("/drake.svg", get(drake_logo))
        // CSS files
        .route("/css/styles.css", get(serve_styles_css))
        // JS files
        .route("/js/utils.js", get(serve_utils_js))
        .route("/js/sessions.js", get(serve_sessions_js))
        .route("/js/llm-configs.js", get(serve_llm_configs_js))
        .route("/js/plugins.js", get(serve_plugins_js))
        .route("/js/tools.js", get(serve_tools_js))
        .route("/js/app.js", get(serve_app_js))

        // WebSocket for live updates (auth required)
        .merge(ws_router)

        // API routes (auth required)
        .nest("/api", api_router)

        .with_state(state)
        .layer(tower_http::trace::TraceLayer::new_for_http());

    // Start server
    let addr = "0.0.0.0:3974";
    info!("🚀 Web UI listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// Health check endpoint
async fn health_check() -> &'static str {
    "OK"
}

/// Serve the main index page
async fn index_page() -> Html<&'static str> {
    Html(include_str!("../../static/index.html"))
}

/// Serve the Drake logo SVG
async fn drake_logo() -> impl axum::response::IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "image/svg+xml")],
        include_str!("../../static/drake.svg"),
    )
}

/// Serve CSS files
async fn serve_styles_css() -> impl axum::response::IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "text/css")],
        include_str!("../../static/css/styles.css"),
    )
}

/// Serve JS files
async fn serve_utils_js() -> impl axum::response::IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "application/javascript")],
        include_str!("../../static/js/utils.js"),
    )
}

async fn serve_sessions_js() -> impl axum::response::IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "application/javascript")],
        include_str!("../../static/js/sessions.js"),
    )
}

async fn serve_llm_configs_js() -> impl axum::response::IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "application/javascript")],
        include_str!("../../static/js/llm-configs.js"),
    )
}

async fn serve_plugins_js() -> impl axum::response::IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "application/javascript")],
        include_str!("../../static/js/plugins.js"),
    )
}

async fn serve_tools_js() -> impl axum::response::IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "application/javascript")],
        include_str!("../../static/js/tools.js"),
    )
}

async fn serve_app_js() -> impl axum::response::IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "application/javascript")],
        include_str!("../../static/js/app.js"),
    )
}

// ============================================================================
// Authentication Middleware
// ============================================================================

/// Authentication middleware - checks Bearer token in Authorization header
async fn auth_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    if !auth_header.starts_with("Bearer ") {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let token = &auth_header[7..]; // Skip "Bearer "
    if token != state.auth_token {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(request).await)
}

// ============================================================================
// Session API Handlers  
// ============================================================================

/// Get a specific session 
async fn get_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    // Extract account ID from query parameters
    let account_id = params.get("account_id").cloned().unwrap_or_default();
    
    if account_id.is_empty() {
        return (StatusCode::BAD_REQUEST, "account_id is required").into_response();
    }
    
    match state.db.get_session(&session_id, &account_id).await {
        Ok(Some((messages, metadata))) => {
            Json(serde_json::json!({
                "session_id": session_id,
                "account_id": account_id,
                "messages": messages,
                "metadata": metadata,
                "created_at": "2026-03-13T00:00:00Z",
                "updated_at": "2026-03-13T00:00:00Z", 
            })).into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Session not found").into_response(),
        Err(e) => {
            error!("Failed to get session: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

/// Get a list of sessions for an account
async fn list_sessions(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    // Extract account ID from query parameters
    let account_id = params.get("account_id").cloned().unwrap_or_default();
    let session_id = params.get("session_id").cloned();
    
    if account_id.is_empty() {
        return (StatusCode::BAD_REQUEST, "account_id is required").into_response();
    }
    
    // If specific session_id is provided, we get only that session
    if let Some(sid) = &session_id {
        match state.db.get_session(sid, &account_id).await {
            Ok(Some((messages, metadata))) => {
                Json(vec![serde_json::json!({
                    "session_id": sid,
                    "account_id": account_id,
                    "messages": messages,
                    "metadata": metadata,
                    "created_at": "2026-03-13T00:00:00Z",
                    "updated_at": "2026-03-13T00:00:00Z", 
                })]).into_response()
            }
            Ok(None) => Json::<Vec<serde_json::Value>>(vec![]).into_response(),
            Err(e) => {
                error!("Failed to get session: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
            }
        }
    } else {
        // List all sessions for the account
        match state.db.list_sessions(&account_id).await {
            Ok(session_ids) => {
                let mut sessions = Vec::new();
                for sid in session_ids {
                    match state.db.get_session(&sid, &account_id).await {
                        Ok(Some((messages, metadata))) => {
                            sessions.push(serde_json::json!({
                                "session_id": sid,
                                "account_id": account_id,
                                "messages": messages,
                                "metadata": metadata,
                                "created_at": "2026-03-13T00:00:00Z",
                                "updated_at": "2026-03-13T00:00:00Z", 
                            }));
                        }
                        Ok(None) => {
                            // Skip sessions that don't exist (shouldn't happen, but just in case)
                        }
                        Err(e) => {
                            error!("Failed to get session {}: {}", sid, e);
                        }
                    }
                }
                Json(sessions).into_response()
            }
            Err(e) => {
                error!("Failed to list sessions: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
            }
        }
    }
}

// ============================================================================
// WebSocket Handler
// ============================================================================

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Response, StatusCode> {
    // Check token from query parameter
    let token = params.get("token").ok_or(StatusCode::UNAUTHORIZED)?;

    if token != &state.auth_token {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(ws.on_upgrade(|socket| handle_websocket(socket, state)))
}

async fn handle_websocket(
    mut socket: axum::extract::ws::WebSocket,
    state: AppState,
) {
    use axum::extract::ws::Message;

    let mut rx = state.update_tx.subscribe();

    info!("WebSocket client connected");

    // Send initial connection message
    if socket.send(Message::Text(
        serde_json::json!({"type": "connected", "message": "Live updates enabled"}).to_string()
    )).await.is_err() {
        return;
    }

    // Forward updates to client
    while let Ok(update) = rx.recv().await {
        let json = match serde_json::to_string(&update) {
            Ok(j) => j,
            Err(e) => {
                error!("Failed to serialize update: {}", e);
                continue;
            }
        };

        if socket.send(Message::Text(json)).await.is_err() {
            break;
        }
    }

    info!("WebSocket client disconnected");
}

// ============================================================================
// LLM Config API Handlers
// ============================================================================

/// List all LLM configurations
async fn list_llm_configs(
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.db.list_llm_configs().await {
        Ok(configs) => Json(configs).into_response(),
        Err(e) => {
            error!("Failed to list LLM configs: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

/// Get a specific LLM configuration
async fn get_llm_config(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.get_llm_config(&id).await {
        Ok(Some(config)) => Json(config).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "Config not found").into_response(),
        Err(e) => {
            error!("Failed to get LLM config: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

/// Request body for creating/updating LLM config
#[derive(Debug, Deserialize)]
struct LlmConfigRequest {
    id: String,
    name: String,
    host: String,
    endpoint: String,
    model: String,
    #[serde(default = "default_context_size")]
    context_size: i32,
    #[serde(default = "default_timeout")]
    timeout_secs: i32,
    #[serde(default)]
    capabilities: Vec<String>,
    #[serde(default)]
    priority: i32,
    #[serde(default = "default_enabled")]
    enabled: bool,
    #[serde(default)]
    metadata: serde_json::Value,
    account_id: Option<String>,
}

fn default_context_size() -> i32 { 32768 }
fn default_timeout() -> i32 { 900 }
fn default_enabled() -> bool { true }

/// Create a new LLM configuration
async fn create_llm_config(
    State(state): State<AppState>,
    Json(req): Json<LlmConfigRequest>,
) -> impl IntoResponse {
    let capabilities = match serde_json::to_string(&req.capabilities) {
        Ok(c) => c,
        Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    };

    let config = LlmConfigRecord {
        id: req.id.clone(),
        name: req.name,
        host: req.host,
        endpoint: req.endpoint,
        model: req.model,
        context_size: req.context_size,
        timeout_secs: req.timeout_secs,
        capabilities,
        priority: req.priority,
        enabled: req.enabled,
        metadata: req.metadata.to_string(),
        account_id: req.account_id,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    match state.db.upsert_llm_config(&config).await {
        Ok(_) => {
            // Broadcast update
            let _ = state.update_tx.send(ConfigUpdate::LlmConfigCreated { id: req.id });
            (StatusCode::CREATED, Json(config)).into_response()
        }
        Err(e) => {
            error!("Failed to create LLM config: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

/// Update an existing LLM configuration
async fn update_llm_config(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<LlmConfigRequest>,
) -> impl IntoResponse {
    // Verify the config exists
    match state.db.get_llm_config(&id).await {
        Ok(Some(_)) => {},
        Ok(None) => return (StatusCode::NOT_FOUND, "Config not found").into_response(),
        Err(e) => {
            error!("Failed to get LLM config: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    }

    let capabilities = match serde_json::to_string(&req.capabilities) {
        Ok(c) => c,
        Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    };

    let config = LlmConfigRecord {
        id: id.clone(),
        name: req.name,
        host: req.host,
        endpoint: req.endpoint,
        model: req.model,
        context_size: req.context_size,
        timeout_secs: req.timeout_secs,
        capabilities,
        priority: req.priority,
        enabled: req.enabled,
        metadata: req.metadata.to_string(),
        account_id: req.account_id,
        created_at: chrono::Utc::now(), // Will be preserved by upsert
        updated_at: chrono::Utc::now(),
    };

    match state.db.upsert_llm_config(&config).await {
        Ok(_) => {
            // Broadcast update
            let _ = state.update_tx.send(ConfigUpdate::LlmConfigUpdated { id });
            Json(config).into_response()
        }
        Err(e) => {
            error!("Failed to update LLM config: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

/// Delete an LLM configuration
async fn delete_llm_config(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.db.delete_llm_config(&id).await {
        Ok(_) => {
            // Broadcast update
            let _ = state.update_tx.send(ConfigUpdate::LlmConfigDeleted { id });
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => {
            error!("Failed to delete LLM config: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

// ============================================================================
// Plugin API Handlers
// ============================================================================

/// Response for installed plugin info
#[derive(Debug, Serialize)]
struct InstalledPlugin {
    name: String,
    version: String,
    description: String,
    author: Option<String>,
    enabled: bool,
}

/// List all installed plugins
async fn list_plugins(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let plugin_dir = &state.plugin_dir;

    if !plugin_dir.exists() {
        return Json(Vec::<InstalledPlugin>::new()).into_response();
    }

    let mut plugins = Vec::new();

    match std::fs::read_dir(plugin_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }

                // Read metadata.json
                let metadata_path = path.join("metadata.json");
                if let Ok(content) = std::fs::read_to_string(&metadata_path) {
                    if let Ok(metadata) = serde_json::from_str::<PackageMetadata>(&content) {
                        plugins.push(InstalledPlugin {
                            name: metadata.name,
                            version: metadata.version,
                            description: metadata.description,
                            author: metadata.author,
                            enabled: true, // TODO: Track enabled/disabled state
                        });
                    }
                }
            }
        }
        Err(e) => {
            error!("Failed to read plugin directory: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    }

    Json(plugins).into_response()
}

/// List available plugins from the registry
async fn list_available_plugins(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let mut client = match RegistryClient::new(state.registry_url.clone(), None, None, false) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to create registry client: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    };

    match client.discover(Some(PackageType::Plugin)).await {
        Ok(packages) => Json(packages).into_response(),
        Err(e) => {
            error!("Failed to discover plugins: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

/// Get available tags for a plugin
async fn get_plugin_tags(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let tags_url = format!("{}/v2/plugins/{}/tags/list", state.registry_url, name);

    match reqwest::get(&tags_url).await {
        Ok(response) => {
            if !response.status().is_success() {
                error!("Failed to fetch tags for plugin {}: {}", name, response.status());
                return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch tags").into_response();
            }

            match response.json::<serde_json::Value>().await {
                Ok(data) => Json(data).into_response(),
                Err(e) => {
                    error!("Failed to parse tags response: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                }
            }
        }
        Err(e) => {
            error!("Failed to fetch tags for plugin {}: {}", name, e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

/// Request body for installing a plugin
#[derive(Debug, Deserialize)]
struct InstallPluginRequest {
    name: String,
    version: String,
}

/// Install a plugin from the registry
async fn install_plugin(
    State(state): State<AppState>,
    Json(req): Json<InstallPluginRequest>,
) -> impl IntoResponse {
    info!("Installing plugin: {} version {}", req.name, req.version);

    let mut client = match RegistryClient::new(state.registry_url.clone(), None, None, false) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to create registry client: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    };

    // Ensure plugin directory exists
    if let Err(e) = std::fs::create_dir_all(&state.plugin_dir) {
        error!("Failed to create plugin directory: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }

    match client.install(
        PackageType::Plugin,
        &req.name,
        &req.version,
        &state.plugin_dir,
    ).await {
        Ok(metadata) => {
            // Broadcast update
            let _ = state.update_tx.send(ConfigUpdate::PluginInstalled { name: req.name });
            (StatusCode::CREATED, Json(metadata)).into_response()
        }
        Err(e) => {
            error!("Failed to install plugin: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

/// Uninstall a plugin
async fn uninstall_plugin(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    info!("Uninstalling plugin: {}", name);

    let plugin_path = state.plugin_dir.join(&name);

    if !plugin_path.exists() {
        return (StatusCode::NOT_FOUND, "Plugin not found").into_response();
    }

    match std::fs::remove_dir_all(&plugin_path) {
        Ok(_) => {
            // Broadcast update
            let _ = state.update_tx.send(ConfigUpdate::PluginUninstalled { name });
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => {
            error!("Failed to uninstall plugin: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

/// List unpublished plugins (scan local directories for plugins with metadata.json)
async fn list_unpublished_plugins(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let mut plugins = Vec::new();

    // Scan /data/plugins directory
    let plugin_dir = &state.plugin_dir;
    info!("Scanning for unpublished plugins in: {:?}", plugin_dir);

    if plugin_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(plugin_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                info!("Checking path: {:?}", path);

                if !path.is_dir() {
                    info!("  Skipping (not a directory)");
                    continue;
                }

                let metadata_path = path.join("metadata.json");
                info!("  Looking for metadata at: {:?}", metadata_path);

                if let Ok(content) = std::fs::read_to_string(&metadata_path) {
                    info!("  Found metadata.json, parsing...");
                    match serde_json::from_str::<PackageMetadata>(&content) {
                        Ok(metadata) => {
                            info!("  ✓ Successfully parsed: {} v{}", metadata.name, metadata.version);
                            plugins.push(serde_json::json!({
                                "name": metadata.name,
                                "version": metadata.version,
                                "description": metadata.description,
                                "path": path.to_string_lossy(),
                            }));
                        }
                        Err(e) => {
                            warn!("  Failed to parse metadata.json: {}", e);
                        }
                    }
                } else {
                    info!("  No metadata.json found");
                }
            }
        }
    } else {
        warn!("Plugin directory does not exist: {:?}", plugin_dir);
    }

    info!("Found {} unpublished plugins", plugins.len());
    Json(plugins).into_response()
}

#[derive(Debug, Deserialize)]
struct PublishPluginRequest {
    path: String,
}

/// Publish a plugin to the registry
async fn publish_plugin(
    State(state): State<AppState>,
    Json(req): Json<PublishPluginRequest>,
) -> impl IntoResponse {
    info!("Publishing plugin from: {}", req.path);

    let plugin_path = std::path::Path::new(&req.path);

    if !plugin_path.exists() {
        return (StatusCode::NOT_FOUND, "Plugin path not found").into_response();
    }

    // Read metadata
    let metadata_path = plugin_path.join("metadata.json");
    let metadata: PackageMetadata = match std::fs::read_to_string(&metadata_path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(m) => m,
            Err(e) => {
                error!("Failed to parse metadata.json: {}", e);
                return (StatusCode::BAD_REQUEST, format!("Invalid metadata.json: {}", e)).into_response();
            }
        },
        Err(e) => {
            error!("Failed to read metadata.json: {}", e);
            return (StatusCode::NOT_FOUND, "metadata.json not found").into_response();
        }
    };

    // Create registry client
    let mut client = match RegistryClient::new(state.registry_url.clone(), None, None, false) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to create registry client: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    };

    // Publish
    match client.publish(plugin_path, metadata.clone()).await {
        Ok(digest) => {
            info!("✓ Published plugin {} version {} (digest: {})", metadata.name, metadata.version, digest);
            Json(serde_json::json!({
                "name": metadata.name,
                "version": metadata.version,
                "digest": digest,
            })).into_response()
        }
        Err(e) => {
            error!("Failed to publish plugin: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

/// Get plugin metadata (from metadata.json file)
async fn get_plugin_metadata(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Response {
    let metadata_path = state.plugin_dir.join(&name).join("metadata.json");

    match tokio::fs::read_to_string(&metadata_path).await {
        Ok(content) => {
            match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(json) => Json(json).into_response(),
                Err(e) => {
                    error!("Failed to parse metadata.json for {}: {}", name, e);
                    (StatusCode::INTERNAL_SERVER_ERROR, "Invalid metadata format").into_response()
                }
            }
        }
        Err(e) => {
            warn!("Metadata not found for plugin {}: {}", name, e);
            (StatusCode::NOT_FOUND, "Metadata not found").into_response()
        }
    }
}

/// Get plugin configuration
async fn get_plugin_config(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Response {
    match state.db.get_plugin_config(&name).await {
        Ok(Some(config)) => {
            // Parse and return as JSON
            match serde_json::from_str::<serde_json::Value>(&config) {
                Ok(json) => Json(json).into_response(),
                Err(e) => {
                    error!("Failed to parse plugin config as JSON: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, "Invalid config format").into_response()
                }
            }
        }
        Ok(None) => {
            // No config found - return empty object
            Json(serde_json::json!({})).into_response()
        }
        Err(e) => {
            error!("Failed to get plugin config: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

/// Update plugin configuration
async fn update_plugin_config(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(config): Json<serde_json::Value>,
) -> Response {
    // Convert JSON to string
    let config_str = match serde_json::to_string(&config) {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to serialize config: {}", e);
            return (StatusCode::BAD_REQUEST, "Invalid JSON").into_response();
        }
    };

    match state.db.set_plugin_config(&name, &config_str).await {
        Ok(_) => {
            info!("Updated config for plugin: {}", name);
            (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))).into_response()
        }
        Err(e) => {
            error!("Failed to update plugin config: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

/// Delete plugin configuration
async fn delete_plugin_config(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Response {
    match state.db.delete_plugin_config(&name).await {
        Ok(deleted) => {
            if deleted {
                info!("Deleted config for plugin: {}", name);
                (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))).into_response()
            } else {
                (StatusCode::NOT_FOUND, "Config not found").into_response()
            }
        }
        Err(e) => {
            error!("Failed to delete plugin config: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

/// Get a secret value (returns existence check only, not the actual value for security)
async fn get_secret(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Response {
    match state.db.get_secret(&key).await {
        Ok(Some(_)) => {
            // Don't return the actual secret value for security
            Json(serde_json::json!({ "exists": true })).into_response()
        }
        Ok(None) => {
            Json(serde_json::json!({ "exists": false })).into_response()
        }
        Err(e) => {
            error!("Failed to check secret: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

/// Set a secret value
async fn set_secret(
    State(state): State<AppState>,
    Path(key): Path<String>,
    body: String,
) -> Response {
    // Expect the body to be a JSON object with a "value" field
    match serde_json::from_str::<serde_json::Value>(&body) {
        Ok(json) => {
            if let Some(value) = json.get("value").and_then(|v| v.as_str()) {
                match state.db.set_secret(&key, value).await {
                    Ok(_) => {
                        info!("Set secret: {}", key);
                        (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))).into_response()
                    }
                    Err(e) => {
                        error!("Failed to set secret: {}", e);
                        (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                    }
                }
            } else {
                (StatusCode::BAD_REQUEST, "Missing 'value' field in request body").into_response()
            }
        }
        Err(e) => {
            error!("Failed to parse secret request: {}", e);
            (StatusCode::BAD_REQUEST, "Invalid JSON").into_response()
        }
    }
}

/// Delete a secret
async fn delete_secret(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Response {
    match state.db.delete_secret(&key).await {
        Ok(deleted) => {
            if deleted {
                info!("Deleted secret: {}", key);
                (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))).into_response()
            } else {
                (StatusCode::NOT_FOUND, "Secret not found").into_response()
            }
        }
        Err(e) => {
            error!("Failed to delete secret: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

// ============================================================================
// Tool Management Handlers
// ============================================================================

/// List all installed tools
async fn list_tools(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let tool_dir = &state.tool_dir;

    if !tool_dir.exists() {
        return Json(Vec::<InstalledPlugin>::new()).into_response();
    }

    let mut tools = Vec::new();

    match std::fs::read_dir(tool_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }

                // Read metadata.json
                let metadata_path = path.join("metadata.json");
                if let Ok(content) = std::fs::read_to_string(&metadata_path) {
                    if let Ok(metadata) = serde_json::from_str::<PackageMetadata>(&content) {
                        tools.push(InstalledPlugin {
                            name: metadata.name,
                            version: metadata.version,
                            description: metadata.description,
                            author: metadata.author,
                            enabled: true, // TODO: Track enabled/disabled state
                        });
                    }
                }
            }
        }
        Err(e) => {
            error!("Failed to read tool directory: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    }

    Json(tools).into_response()
}

/// List available tools from the registry
async fn list_available_tools(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let mut client = match RegistryClient::new(state.registry_url.clone(), None, None, false) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to create registry client: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    };

    match client.discover(Some(PackageType::Tool)).await {
        Ok(packages) => Json(packages).into_response(),
        Err(e) => {
            error!("Failed to discover tools: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

/// Get available tags for a tool
async fn get_tool_tags(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let tags_url = format!("{}/v2/tools/{}/tags/list", state.registry_url, name);

    match reqwest::get(&tags_url).await {
        Ok(response) => {
            if !response.status().is_success() {
                error!("Failed to fetch tags for tool {}: {}", name, response.status());
                return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch tags").into_response();
            }

            match response.json::<serde_json::Value>().await {
                Ok(data) => Json(data).into_response(),
                Err(e) => {
                    error!("Failed to parse tags response: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                }
            }
        }
        Err(e) => {
            error!("Failed to fetch tags for tool {}: {}", name, e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

/// Request body for installing a tool
#[derive(Debug, Deserialize)]
struct InstallToolRequest {
    name: String,
    version: String,
}

/// Install a tool from the registry
async fn install_tool(
    State(state): State<AppState>,
    Json(req): Json<InstallToolRequest>,
) -> impl IntoResponse {
    info!("Installing tool: {} version {}", req.name, req.version);

    let mut client = match RegistryClient::new(state.registry_url.clone(), None, None, false) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to create registry client: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    };

    // Ensure tool directory exists
    if let Err(e) = std::fs::create_dir_all(&state.tool_dir) {
        error!("Failed to create tool directory: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }

    match client.install(
        PackageType::Tool,
        &req.name,
        &req.version,
        &state.tool_dir,
    ).await {
        Ok(metadata) => {
            // Broadcast update
            let _ = state.update_tx.send(ConfigUpdate::ToolInstalled { name: req.name });
            (StatusCode::CREATED, Json(metadata)).into_response()
        }
        Err(e) => {
            error!("Failed to install tool: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}


/// Uninstall a tool
async fn uninstall_tool(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    info!("Uninstalling tool: {}", name);

    let tool_path = state.tool_dir.join(&name);

    if !tool_path.exists() {
        return (StatusCode::NOT_FOUND, "Tool not found").into_response();
    }

    match std::fs::remove_dir_all(&tool_path) {
        Ok(_) => {
            // Broadcast update
            let _ = state.update_tx.send(ConfigUpdate::ToolUninstalled { name });
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => {
            error!("Failed to uninstall tool: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

/// List unpublished tools (scan local directories for tools with metadata.json)
async fn list_unpublished_tools(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let mut tools = Vec::new();

    // Scan /data/tools directory
    let tool_dir = &state.tool_dir;
    info!("Scanning for unpublished tools in: {:?}", tool_dir);

    if tool_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(tool_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                info!("Checking path: {:?}", path);

                if !path.is_dir() {
                    info!("  Skipping (not a directory)");
                    continue;
                }

                let metadata_path = path.join("metadata.json");
                info!("  Looking for metadata at: {:?}", metadata_path);

                if let Ok(content) = std::fs::read_to_string(&metadata_path) {
                    info!("  Found metadata.json, parsing...");
                    match serde_json::from_str::<PackageMetadata>(&content) {
                        Ok(metadata) => {
                            info!("  ✓ Successfully parsed: {} v{}", metadata.name, metadata.version);
                            tools.push(serde_json::json!({
                                "name": metadata.name,
                                "version": metadata.version,
                                "description": metadata.description,
                                "path": path.to_string_lossy(),
                            }));
                        }
                        Err(e) => {
                            warn!("  Failed to parse metadata.json: {}", e);
                        }
                    }
                } else {
                    info!("  No metadata.json found");
                }
            }
        }
    } else {
        warn!("Tool directory does not exist: {:?}", tool_dir);
    }

    info!("Found {} unpublished tools", tools.len());
    Json(tools).into_response()
}

#[derive(Debug, Deserialize)]
struct PublishToolRequest {
    path: String,
}

/// Publish a tool to the registry
async fn publish_tool(
    State(state): State<AppState>,
    Json(req): Json<PublishToolRequest>,
) -> impl IntoResponse {
    info!("Publishing tool from: {}", req.path);

    let tool_path = std::path::Path::new(&req.path);

    if !tool_path.exists() {
        return (StatusCode::NOT_FOUND, "Tool path not found").into_response();
    }

    // Read metadata
    let metadata_path = tool_path.join("metadata.json");
    let metadata: PackageMetadata = match std::fs::read_to_string(&metadata_path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(m) => m,
            Err(e) => {
                error!("Failed to parse metadata.json: {}", e);
                return (StatusCode::BAD_REQUEST, format!("Invalid metadata.json: {}", e)).into_response();
            }
        },
        Err(e) => {
            error!("Failed to read metadata.json: {}", e);
            return (StatusCode::NOT_FOUND, "metadata.json not found").into_response();
        }
    };

    // Create registry client
    let mut client = match RegistryClient::new(state.registry_url.clone(), None, None, false) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to create registry client: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    };

    // Publish
    match client.publish(tool_path, metadata.clone()).await {
        Ok(digest) => {
            info!("✓ Published tool {} version {} (digest: {})", metadata.name, metadata.version, digest);
            Json(serde_json::json!({
                "name": metadata.name,
                "version": metadata.version,
                "digest": digest,
            })).into_response()
        }
        Err(e) => {
            error!("Failed to publish tool: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

/// Get tool metadata (from metadata.json file)
async fn get_tool_metadata(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Response {
    let metadata_path = state.tool_dir.join(&name).join("metadata.json");

    match tokio::fs::read_to_string(&metadata_path).await {
        Ok(content) => {
            match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(json) => Json(json).into_response(),
                Err(e) => {
                    error!("Failed to parse metadata.json for {}: {}", name, e);
                    (StatusCode::INTERNAL_SERVER_ERROR, "Invalid metadata format").into_response()
                }
            }
        }
        Err(e) => {
            warn!("Metadata not found for tool {}: {}", name, e);
            (StatusCode::NOT_FOUND, "Metadata not found").into_response()
        }
    }
}

/// Get tool configuration
async fn get_tool_config(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Response {
    // Tools use the same config storage as plugins, just with a "tool." prefix
    let scope = format!("tool.{}", name);

    match state.db.get_plugin_config(&scope).await {
        Ok(Some(config)) => {
            // Parse and return as JSON
            match serde_json::from_str::<serde_json::Value>(&config) {
                Ok(json) => Json(json).into_response(),
                Err(e) => {
                    error!("Failed to parse tool config as JSON: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, "Invalid config format").into_response()
                }
            }
        }
        Ok(None) => {
            // No config found - return empty object
            Json(serde_json::json!({})).into_response()
        }
        Err(e) => {
            error!("Failed to get tool config: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

/// Update tool configuration
async fn update_tool_config(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(config): Json<serde_json::Value>,
) -> Response {
    // Tools use the same config storage as plugins, just with a "tool." prefix
    let scope = format!("tool.{}", name);

    // Convert JSON to string
    let config_str = match serde_json::to_string(&config) {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to serialize config: {}", e);
            return (StatusCode::BAD_REQUEST, "Invalid JSON").into_response();
        }
    };

    match state.db.set_plugin_config(&scope, &config_str).await {
        Ok(_) => {
            info!("Updated config for tool: {}", name);
            (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))).into_response()
        }
        Err(e) => {
            error!("Failed to update tool config: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

/// Delete tool configuration
async fn delete_tool_config(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Response {
    // Tools use the same config storage as plugins, just with a "tool." prefix
    let scope = format!("tool.{}", name);

    match state.db.delete_plugin_config(&scope).await {
        Ok(deleted) => {
            if deleted {
                info!("Deleted config for tool: {}", name);
                (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))).into_response()
            } else {
                (StatusCode::NOT_FOUND, "Config not found").into_response()
            }
        }
        Err(e) => {
            error!("Failed to delete tool config: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

