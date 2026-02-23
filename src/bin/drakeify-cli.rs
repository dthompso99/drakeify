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

    /// List installed plugins or tools
    List {
        /// Type of package (plugin or tool)
        #[arg(short, long)]
        package_type: String,
    },

    /// Remove an installed plugin or tool
    Remove {
        /// Type of package (plugin or tool)
        #[arg(short, long)]
        package_type: String,

        /// Package name
        #[arg(short, long)]
        name: String,
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
    // Check if we're being invoked as /bin/sh without arguments
    // This happens when k9s tries to open a shell
    let args: Vec<String> = std::env::args().collect();
    let program_name = args.get(0).map(|s| s.as_str()).unwrap_or("");

    // If invoked as "sh" or "/bin/sh" without -c, provide a helpful message and exit
    if (program_name.ends_with("/sh") || program_name == "sh") && args.len() == 1 {
        eprintln!("drakeify-cli: This is a minimal shell for k9s compatibility.");
        eprintln!("For interactive chat mode, use: drakeify-cli chat");
        eprintln!("For package management, use: drakeify-cli --help");
        std::process::exit(0);
    }

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
        Some(Commands::Remove { package_type, name }) => {
            handle_remove(&config, package_type, name).await
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

/// Handle the list command - lists locally installed packages
async fn handle_list(
    _config: &DrakeifyConfig,
    package_type: String,
) -> Result<()> {
    use std::fs;
    use std::path::Path;

    let pkg_type = match package_type.to_lowercase().as_str() {
        "plugin" => PackageType::Plugin,
        "tool" => PackageType::Tool,
        _ => return Err(anyhow::anyhow!("Invalid package type. Must be 'plugin' or 'tool'")),
    };

    let dir_name = if pkg_type == PackageType::Plugin {
        "plugins"
    } else {
        "tools"
    };

    let dir_path = Path::new(dir_name);

    if !dir_path.exists() {
        println!("No {}s installed (directory {} does not exist)", package_type, dir_name);
        return Ok(());
    }

    let mut packages = Vec::new();

    for entry in fs::read_dir(dir_path)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let metadata_file = path.join("metadata.json");
            if metadata_file.exists() {
                let metadata_content = fs::read_to_string(&metadata_file)?;
                let metadata: PackageMetadata = serde_json::from_str(&metadata_content)?;
                packages.push(metadata);
            }
        }
    }

    if packages.is_empty() {
        println!("No {}s installed", package_type);
    } else {
        println!("Installed {}s:", package_type);
        for pkg in packages {
            println!("  📦 {} v{}", pkg.name, pkg.version);
            println!("     {}", pkg.description);
            if let Some(author) = pkg.author {
                println!("     Author: {}", author);
            }
            println!();
        }
    }

    Ok(())
}

/// Handle the remove command
async fn handle_remove(
    _config: &DrakeifyConfig,
    package_type: String,
    name: String,
) -> Result<()> {
    use std::fs;
    use std::path::Path;

    let pkg_type = match package_type.to_lowercase().as_str() {
        "plugin" => PackageType::Plugin,
        "tool" => PackageType::Tool,
        _ => return Err(anyhow::anyhow!("Invalid package type. Must be 'plugin' or 'tool'")),
    };

    let dir_name = if pkg_type == PackageType::Plugin {
        "plugins"
    } else {
        "tools"
    };

    let package_path = Path::new(dir_name).join(&name);

    if !package_path.exists() {
        return Err(anyhow::anyhow!("{} '{}' is not installed", package_type, name));
    }

    // Read metadata before removing
    let metadata_file = package_path.join("metadata.json");
    let metadata: PackageMetadata = if metadata_file.exists() {
        let metadata_content = fs::read_to_string(&metadata_file)?;
        serde_json::from_str(&metadata_content)?
    } else {
        return Err(anyhow::anyhow!("Package metadata not found for '{}'", name));
    };

    // Remove the package directory
    fs::remove_dir_all(&package_path)?;

    println!("✓ Successfully removed {} '{}' v{}", package_type, name, metadata.version);

    Ok(())
}

/// Handle shell command execution (for k9s compatibility)
async fn handle_shell_command(command: &str) -> Result<()> {
    let config = DrakeifyConfig::load_with_env()?;

    // If the command starts with /, treat it as a slash command
    if command.trim().starts_with('/') {
        // Execute the slash command
        if handle_slash_command(command.trim(), &config).await? {
            println!("Command executed successfully");
        }
        // After executing slash command, enter interactive mode
        info!("Entering interactive mode...");
        run_interactive_mode(&config).await
    } else {
        // For non-slash commands, just enter interactive mode
        info!("Entering interactive mode (shell command '{}' ignored)", command);
        run_interactive_mode(&config).await
    }
}

/// Handle slash commands in interactive mode
/// Returns true if the command was handled, false if it should be sent to LLM
async fn handle_slash_command(input: &str, config: &DrakeifyConfig) -> Result<bool> {
    if !input.starts_with('/') {
        return Ok(false);
    }

    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(false);
    }

    match parts[0] {
        "/packages" => {
            if parts.len() < 2 {
                println!("Usage: /packages <ls|publish|install|remove|search> [args...]");
                println!("  /packages ls <plugin|tool>");
                println!("  /packages publish <plugin|tool> <path> <name> <version> <description> [author] [license]");
                println!("  /packages install <plugin|tool> <name> <version>");
                println!("  /packages remove <plugin|tool> <name>");
                println!("  /packages search <plugin|tool> <query>");
                return Ok(true);
            }

            match parts[1] {
                "ls" | "list" => {
                    if parts.len() < 3 {
                        println!("Usage: /packages ls <plugin|tool>");
                        return Ok(true);
                    }
                    handle_list(config, parts[2].to_string()).await?;
                }
                "publish" => {
                    if parts.len() < 7 {
                        println!("Usage: /packages publish <plugin|tool> <path> <name> <version> <description> [author] [license]");
                        return Ok(true);
                    }
                    let author = parts.get(7).map(|s| s.to_string());
                    let license = parts.get(8).map(|s| s.to_string());
                    handle_publish(
                        config,
                        parts[2].to_string(),
                        parts[3].to_string(),
                        parts[4].to_string(),
                        parts[5].to_string(),
                        parts[6].to_string(),
                        author,
                        license,
                    ).await?;
                }
                "install" => {
                    if parts.len() < 5 {
                        println!("Usage: /packages install <plugin|tool> <name> <version>");
                        return Ok(true);
                    }
                    handle_install(
                        config,
                        parts[2].to_string(),
                        parts[3].to_string(),
                        parts[4].to_string(),
                    ).await?;
                }
                "remove" | "rm" => {
                    if parts.len() < 4 {
                        println!("Usage: /packages remove <plugin|tool> <name>");
                        return Ok(true);
                    }
                    handle_remove(
                        config,
                        parts[2].to_string(),
                        parts[3].to_string(),
                    ).await?;
                }
                "search" => {
                    if parts.len() < 4 {
                        println!("Usage: /packages search <plugin|tool> <query>");
                        return Ok(true);
                    }
                    // For now, search is the same as list
                    println!("Searching for {}s matching '{}'...", parts[2], parts[3]);
                    handle_list(config, parts[2].to_string()).await?;
                }
                _ => {
                    println!("Unknown packages command: {}", parts[1]);
                    println!("Available commands: ls, publish, install, remove, search");
                }
            }
            Ok(true)
        }
        "/help" => {
            println!("\nAvailable slash commands:");
            println!("  /packages ls <plugin|tool>                                    - List installed packages");
            println!("  /packages publish <type> <path> <name> <ver> <desc> [author] - Publish a package");
            println!("  /packages install <plugin|tool> <name> <version>              - Install a package");
            println!("  /packages remove <plugin|tool> <name>                         - Remove an installed package");
            println!("  /packages search <plugin|tool> <query>                        - Search for packages");
            println!("  /help                                                         - Show this help");
            println!("\nSlash commands are executed locally and do not go to the LLM.\n");
            Ok(true)
        }
        _ => {
            println!("Unknown command: {}. Type /help for available commands.", parts[0]);
            Ok(true)
        }
    }
}

/// Run interactive chat mode - acts as a pure HTTP client to the proxy
async fn run_interactive_mode(config: &DrakeifyConfig) -> Result<()> {
    info!("🤖 Drakeify Interactive Mode");
    info!("Connecting to proxy at http://{}:{}", config.proxy_host, config.proxy_port);
    info!("Type 'exit' or 'quit' to end the conversation\n");

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

    // Create HTTP client for talking to the proxy
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(900))
        .build()?;

    let proxy_url = format!("http://{}:{}/v1/chat/completions", config.proxy_host, config.proxy_port);

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

        // Check for slash commands
        if handle_slash_command(user_input, config).await? {
            // Command was handled, continue to next iteration
            println!();
            continue;
        }

        // Add user message to conversation
        let user_message = OllamaMessage {
            role: "user".to_string(),
            content: user_input.to_string(),
            tool_calls: vec![],
        };
        conversation_messages.push(user_message.clone());

        // Send request to proxy
        print!("\nAssistant: ");
        std::io::stdout().flush()?;

        let request_body = serde_json::json!({
            "model": config.llm_model.clone(),
            "messages": conversation_messages,
            "stream": false,
        });

        let response = client
            .post(&proxy_url)
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            eprintln!("Error from proxy: {}", error_text);
            conversation_messages.pop(); // Remove the user message we just added
            continue;
        }

        let response_json: serde_json::Value = response.json().await?;

        // Extract assistant message from response
        let assistant_content = response_json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("(no response)")
            .to_string();

        println!("{}\n", assistant_content);

        // Add assistant response to conversation
        let assistant_message = OllamaMessage {
            role: "assistant".to_string(),
            content: assistant_content,
            tool_calls: vec![],
        };
        conversation_messages.push(assistant_message);

        // Save entire conversation to session
        session_manager.update_messages(conversation_messages.clone())?;
    }

    Ok(())
}

