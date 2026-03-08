// Drakeify - Proxy-only mode
// This binary runs only the HTTP proxy server

use anyhow::Result;
use drakeify::{DrakeifyConfig, init_logging, proxy, Database, SchedulerConfig, start_scheduler, LlmConfig};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    let config = DrakeifyConfig::load_with_env()?;

    // Initialize logging
    init_logging(&config)?;

    info!("Drakeify proxy starting up");

    // Initialize database
    let db = Database::connect(&config.database_url).await?;
    db.migrate().await?;

    // Create JavaScript runtime configuration
    let js_config = drakeify::JsRuntimeConfig {
        allow_http: config.allow_http,
        http_timeout_secs: config.http_timeout_secs,
        http_max_response_size: config.http_max_response_size,
        allowed_domains: config.allowed_domains.clone(),
    };

    // Start scheduler if enabled
    if config.scheduler_enabled {
        info!("🕐 Starting scheduled task runner");
        info!("   Pod ID: {}", config.scheduler_pod_id);
        info!("   Poll interval: {}s", config.scheduler_poll_interval_secs);

        let scheduler_config = SchedulerConfig {
            poll_interval_secs: config.scheduler_poll_interval_secs,
            pod_id: config.scheduler_pod_id.clone(),
            llm_model: config.llm_model.clone(),
            llm_config: LlmConfig {
                host: config.llm_host.clone(),
                endpoint: config.llm_endpoint.clone(),
                timeout_secs: 900,
            },
            context_size: config.context_size,
            js_config: js_config.clone(),
            enabled_tools: config.enabled_tools.clone(),
            disabled_tools: config.disabled_tools.clone(),
            enabled_plugins: config.enabled_plugins.clone(),
            disabled_plugins: config.disabled_plugins.clone(),
        };

        let db_clone = db.clone();
        tokio::spawn(async move {
            if let Err(e) = start_scheduler(db_clone, scheduler_config).await {
                tracing::error!("Scheduler error: {}", e);
            }
        });
    } else {
        info!("⏸️  Scheduled task runner is disabled");
    }

    // Start proxy server
    info!("🌐 Starting proxy server on {}:{}", config.proxy_host, config.proxy_port);

    proxy::start_proxy_server(
        config.proxy_host.clone(),
        config.proxy_port,
        config.llm_host.clone(),
        config.llm_model.clone(),
        config.llm_endpoint.clone(),
        config.context_size,
        config.stream,
        js_config,
        config.enabled_tools.clone(),
        config.disabled_tools.clone(),
        config.enabled_plugins.clone(),
        config.disabled_plugins.clone(),
        db,
    ).await?;

    Ok(())
}

