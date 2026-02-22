// Drakeify - Proxy-only mode
// This binary runs only the HTTP proxy server

use anyhow::Result;
use drakeify::{DrakeifyConfig, init_logging, proxy};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    let config = DrakeifyConfig::load_with_env()?;

    // Initialize logging
    init_logging(&config)?;

    info!("Drakeify proxy starting up");

    // Create JavaScript runtime configuration
    let js_config = drakeify::JsRuntimeConfig {
        allow_http: config.allow_http,
        http_timeout_secs: config.http_timeout_secs,
        http_max_response_size: config.http_max_response_size,
        allowed_domains: config.allowed_domains.clone(),
    };

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
    ).await?;

    Ok(())
}

