// Drakeify CLI - Interactive mode and plugin/tool management
// This binary handles:
// - Interactive chat mode
// - Plugin/tool publishing
// - Plugin/tool installation
// - Shell compatibility for k9s

use anyhow::Result;
use clap::{Parser, Subcommand};
use drakeify::*;
use tracing::{info, warn};
use std::io::Write;

/// Drakeify CLI - Interactive AI Agent and Plugin/Tool Manager
#[derive(Parser, Debug)]
#[command(name = "drakeify-cli")]
#[command(about = "Interactive AI Agent and Plugin/Tool Manager", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run interactive chat mode (default)
    Chat,

    /// Publish a plugin or tool to the registry
    Publish {
        /// Type of package (plugin or tool)
        #[arg(short, long)]
        package_type: String,

        /// Path to the package directory
        #[arg(short = 'd', long)]
        path: String,

        /// Package name
        #[arg(short, long)]
        name: String,

        /// Package version
        #[arg(short, long)]
        version: String,

        /// Package description
        #[arg(short = 'D', long)]
        description: String,

        /// Author name (optional)
        #[arg(short, long)]
        author: Option<String>,

        /// License (optional)
        #[arg(short, long)]
        license: Option<String>,
    },

    /// Install a plugin or tool from the registry
    Install {
        /// Type of package (plugin or tool)
        #[arg(short, long)]
        package_type: String,

        /// Package name
        #[arg(short, long)]
        name: String,

        /// Package version
        #[arg(short, long)]
        version: String,
    },

    /// List available plugins or tools in the registry
    List {
        /// Type of package (plugin or tool)
        #[arg(short, long)]
        package_type: String,
    },

    /// Execute a shell command (for k9s compatibility)
    #[command(name = "-c")]
    ShellCommand {
        /// Command to execute
        command: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = DrakeifyConfig::load_with_env()?;

    // Initialize logging
    init_logging(&config)?;

    // Handle CLI commands
    match cli.command {
        Some(Commands::Publish { package_type, path, name, version, description, author, license }) => {
            handle_publish(&config, package_type, path, name, version, description, author, license).await
        }
        Some(Commands::Install { package_type, name, version }) => {
            handle_install(&config, package_type, name, version).await
        }
        Some(Commands::List { package_type }) => {
            handle_list(&config, package_type).await
        }
        Some(Commands::ShellCommand { command }) => {
            handle_shell_command(&command).await
        }
        Some(Commands::Chat) | None => {
            run_interactive_mode(&config).await
        }
    }
}

/// Handle the publish command
async fn handle_publish(
    config: &DrakeifyConfig,
    package_type: String,
    path: String,
    name: String,
    version: String,
    description: String,
    author: Option<String>,
    license: Option<String>,
) -> Result<()> {
    let pkg_type = match package_type.to_lowercase().as_str() {
        "plugin" => PackageType::Plugin,
        "tool" => PackageType::Tool,
        _ => return Err(anyhow::anyhow!("Invalid package type. Must be 'plugin' or 'tool'")),
    };

    let metadata = PackageMetadata {
        package_type: pkg_type,
        name: name.clone(),
        version: version.clone(),
        description,
        author,
        license,
        homepage: None,
        dependencies: Default::default(),
        drakeify_version: Some(">=0.1.0".to_string()),
        tags: vec![],
        created: chrono::Utc::now().to_rfc3339(),
    };

    let mut client = RegistryClient::new(
        config.registry_url.clone(),
        config.registry_username.clone(),
        config.registry_password.clone(),
        config.registry_insecure,
    )?;

    let package_path = std::path::PathBuf::from(path);
    client.publish(&package_path, metadata).await?;

    println!("✓ Successfully published {}/{} version {}", package_type, name, version);

    Ok(())
}

/// Handle the install command
async fn handle_install(
    config: &DrakeifyConfig,
    package_type: String,
    name: String,
    version: String,
) -> Result<()> {
    let pkg_type = match package_type.to_lowercase().as_str() {
        "plugin" => PackageType::Plugin,
        "tool" => PackageType::Tool,
        _ => return Err(anyhow::anyhow!("Invalid package type. Must be 'plugin' or 'tool'")),
    };

    let install_dir = match pkg_type {
        PackageType::Plugin => std::path::PathBuf::from("plugins"),
        PackageType::Tool => std::path::PathBuf::from("tools"),
    };

    let mut client = RegistryClient::new(
        config.registry_url.clone(),
        config.registry_username.clone(),
        config.registry_password.clone(),
        config.registry_insecure,
    )?;

    let metadata = client.install(pkg_type, &name, &version, &install_dir).await?;

    println!("✓ Successfully installed {}/{} version {}", package_type, name, metadata.version);
    println!("  Description: {}", metadata.description);
    if let Some(author) = metadata.author {
        println!("  Author: {}", author);
    }

    Ok(())
}

/// Handle the list command
async fn handle_list(
    config: &DrakeifyConfig,
    package_type: String,
) -> Result<()> {
    let pkg_type = match package_type.to_lowercase().as_str() {
        "plugin" => PackageType::Plugin,
        "tool" => PackageType::Tool,
        _ => return Err(anyhow::anyhow!("Invalid package type. Must be 'plugin' or 'tool'")),
    };

    let mut client = RegistryClient::new(
        config.registry_url.clone(),
        config.registry_username.clone(),
        config.registry_password.clone(),
        config.registry_insecure,
    )?;

    let packages = client.list(pkg_type).await?;

    if packages.is_empty() {
        println!("No {}s found in registry", package_type);
    } else {
        println!("Available {}s:", package_type);
        for pkg in packages {
            println!("  - {}", pkg);
        }
    }

    Ok(())
}

/// Handle shell command execution (for k9s compatibility)
async fn handle_shell_command(command: &str) -> Result<()> {
    use std::process::Command;

    // Execute the command using sh
    let output = Command::new("sh")
        .arg("-c")
        .arg(command)
        .output()?;

    // Print stdout
    std::io::stdout().write_all(&output.stdout)?;

    // Print stderr
    std::io::stderr().write_all(&output.stderr)?;

    // Exit with the same code as the command
    std::process::exit(output.status.code().unwrap_or(1));
}

/// Run interactive chat mode
async fn run_interactive_mode(config: &DrakeifyConfig) -> Result<()> {
    info!("🤖 Drakeify Interactive Mode");
    info!("Type 'exit' or 'quit' to end the conversation\n");

    // Create JavaScript runtime configuration
    let js_config = JsRuntimeConfig {
        allow_http: config.allow_http,
        http_timeout_secs: config.http_timeout_secs,
        http_max_response_size: config.http_max_response_size,
        allowed_domains: config.allowed_domains.clone(),
    };

    // Initialize tool registry and auto-discover tools
    let mut tool_registry = ToolRegistry::new(
        js_config.clone(),
        config.enabled_tools.clone(),
        config.disabled_tools.clone()
    )?;
    tool_registry.load_tools_from_dir("tools")?;

    let registered_tools = tool_registry.list_tools();
    info!("Registered {} tools: {:?}", registered_tools.len(), registered_tools);

    // Initialize plugin registry and auto-discover plugins
    let mut plugin_registry = PluginRegistry::new(
        js_config.clone(),
        config.enabled_plugins.clone(),
        config.disabled_plugins.clone()
    )?;
    plugin_registry.load_plugins_from_dir("plugins")?;

    let registered_plugins = plugin_registry.get_plugins();
    info!("Registered {} plugins", registered_plugins.len());
    for plugin in registered_plugins {
        info!("  - {} (priority: {}, hooks: {:?})", plugin.name, plugin.priority, plugin.hooks);
    }

    let llm_config = LlmConfig {
        host: config.llm_host.clone(),
        endpoint: config.llm_endpoint.clone(),
        timeout_secs: 900,
    };

    // Initialize session manager
    let mut session_manager = SessionManager::new(&config.sessions_dir, config.auto_save)?;

    // Load existing session or create new one
    if let Some(session_id) = &config.session_id {
        if !session_id.is_empty() {
            match session_manager.load_session(session_id) {
                Ok(_) => {
                    info!("📂 Loaded session: {}", session_id);
                }
                Err(e) => {
                    warn!("⚠️  Failed to load session {}: {}", session_id, e);
                    info!("Creating new session instead...");
                    let new_id = session_manager.new_session()?;
                    info!("📝 Created new session: {}", new_id);
                }
            }
        } else {
            let new_id = session_manager.new_session()?;
            info!("📝 Created new session: {}", new_id);
        }
    } else {
        let new_id = session_manager.new_session()?;
        info!("📝 Created new session: {}", new_id);
    }

    // System message for all conversations (from config)
    let system_message = OllamaMessage {
        role: "system".to_string(),
        content: config.system_prompt.clone(),
        tool_calls: vec![],
    };

    // Load messages from session, or start with system message if new session
    let mut conversation_messages = session_manager.get_messages();
    if conversation_messages.is_empty() {
        conversation_messages.push(system_message);
        session_manager.update_messages(conversation_messages.clone())?;
    }

    loop {
        // Get user input
        print!("You: ");
        std::io::stdout().flush()?;

        let mut user_input = String::new();
        std::io::stdin().read_line(&mut user_input)?;
        let user_input = user_input.trim();

        // Check for exit commands
        if user_input.eq_ignore_ascii_case("exit") || user_input.eq_ignore_ascii_case("quit") {
            info!("\n👋 Goodbye!");
            break;
        }

        if user_input.is_empty() {
            continue;
        }

        // Add user message to conversation
        let user_message = OllamaMessage {
            role: "user".to_string(),
            content: user_input.to_string(),
            tool_calls: vec![],
        };
        conversation_messages.push(user_message);

        // Run conversation with tool execution loop
        print!("\nAssistant: ");
        std::io::stdout().flush()?;

        let assistant_response = run_conversation(
            &mut conversation_messages,
            &config,
            &llm_config,
            &tool_registry,
            &plugin_registry,
        ).await?;

        // Save entire conversation to session (includes all tool calls and responses)
        session_manager.update_messages(conversation_messages.clone())?;

        // Execute on_conversation_turn plugin hook
        let turn_data = serde_json::json!({
            "user_message": user_input,
            "assistant_message": assistant_response,
        });
        plugin_registry.execute_hook("on_conversation_turn", turn_data)?;
    }

    Ok(())
}

