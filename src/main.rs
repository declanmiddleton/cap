use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use tracing::error;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod cli;
mod core;
mod modules;
mod shell;
mod web;

use cli::banner::display_banner;
use core::{config::Config, session::SessionManager};
use shell::{ShellListener, ShellSessionManager, InteractiveTerminal, InterfaceSelector, get_port_input};
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "cap")]
#[command(about = "Modern terminal-based reverse shell handler", long_about = None)]
#[command(after_help = "CAP is a shell handler built for reliability, clarity, and flow.\nFocus: listener and session management only.")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Start interactive shell listener
    #[command(alias = "l")]
    Listen {
        #[arg(short, long, default_value = "0.0.0.0")]
        host: String,

        #[arg(short, long, default_value = "4444")]
        port: u16,
    },

    /// List active shell sessions
    #[command(alias = "ls")]
    Sessions,

    /// Attach to a backgrounded session
    Attach {
        /// Session ID to attach to
        id: String,
    },

    /// Kill a session
    Kill {
        /// Session ID to terminate
        id: String,
    },

    /// Add a note to a session
    Note {
        /// Session ID
        id: String,
        /// Note text
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        text: Vec<String>,
    },

    /// Legacy web testing (deprecated - use dedicated tools)
    #[command(hide = true)]
    Web {
        #[command(subcommand)]
        action: WebAction,
    },

    /// Legacy module commands (deprecated)
    #[command(hide = true)]
    Modules,

    #[command(hide = true)]
    Module {
        #[arg(short, long)]
        name: String,
        #[arg(short, long)]
        target: String,
        #[arg(long)]
        wordlist: Option<String>,
        #[arg(long)]
        threads: Option<usize>,
        #[arg(long)]
        verbose: bool,
        #[arg(long, value_delimiter = ',')]
        status_codes: Option<Vec<u16>>,
        #[arg(long, value_delimiter = ',')]
        exclude_codes: Option<Vec<u16>>,
    },

    #[command(hide = true)]
    Generate {
        #[arg(short, long)]
        module: String,
        #[arg(short, long)]
        target: String,
        #[arg(short, long)]
        output: Option<String>,
    },

    #[command(hide = true)]
    Scope {
        #[command(subcommand)]
        action: ScopeAction,
    },

    #[command(hide = true)]
    Audit {
        #[arg(long)]
        session_id: Option<String>,
        #[arg(long)]
        export: Option<String>,
    },

    #[command(hide = true)]
    Init {
        #[arg(short, long)]
        name: String,
    },

    #[command(hide = true)]
    Wordlists {
        #[arg(short, long)]
        search: Option<String>,
    },

    #[command(hide = true)]
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },

    #[command(hide = true)]
    Shell {
        #[command(subcommand)]
        action: ShellAction,
    },
}

#[derive(Subcommand)]
enum SessionAction {
    /// List all active sessions
    List,
    /// Create a new session
    New { name: String },
    /// Attach to a session
    Attach { id: String },
    /// Terminate a session
    Kill { id: String },
}

#[derive(Subcommand)]
enum ShellAction {
    /// List active shell sessions
    List,
    /// Interact with a shell session
    Interact {
        #[arg(short, long)]
        id: String,
    },
    /// Kill a shell session
    Kill {
        #[arg(short, long)]
        id: String,
    },
    /// Upgrade shell to PTY
    Upgrade {
        #[arg(short, long)]
        id: String,
    },
    /// Run a command on a shell session
    Run {
        #[arg(short, long)]
        id: String,
        
        #[arg(short, long)]
        command: String,
    },
}

#[derive(Subcommand)]
enum WebAction {
    /// List all available web modules
    List,
    
    /// Show detailed module information
    Info {
        #[arg(short, long)]
        module: String,
    },
    
    /// Execute a web module
    Run {
        #[arg(short, long)]
        module: String,
        
        #[arg(short, long)]
        request: Option<String>,
        
        #[arg(short, long)]
        injection_point: Option<String>,
        
        #[arg(short, long)]
        url: Option<String>,
        
        #[arg(long)]
        lhost: Option<String>,
        
        #[arg(long)]
        lport: Option<String>,
        
        #[arg(long)]
        dry_run: bool,
        
        #[arg(long)]
        confirm_each: bool,
    },
}

#[derive(Subcommand)]
enum ScopeAction {
    /// Add a target to authorized scope
    Add { target: String },
    /// Remove a target from scope
    Remove { target: String },
    /// List all authorized targets
    List,
    /// Verify if a target is in scope
    Check { target: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Show full banner if help is requested or no command given
    let args: Vec<String> = std::env::args().collect();
    let show_full_banner = args.len() == 1 
        || args.contains(&"--help".to_string()) 
        || args.contains(&"-h".to_string())
        || args.contains(&"help".to_string());
    
    if show_full_banner {
        display_banner();
    }

    // Parse CLI arguments
    let cli = Cli::parse();

    // Check if this is a listen command or no command (default listen) - suppress logging for clean output
    let is_listen = matches!(cli.command, Some(Commands::Listen { .. })) || cli.command.is_none();

    // Initialize logging
    let log_level = if is_listen {
        "error"  // Only errors for listen command
    } else if cli.verbose {
        "debug"
    } else {
        "info"
    };
    
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("cap={},tower_http=debug", log_level).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration (use defaults if file doesn't exist)
    let config = Config::load_or_default("config/default.toml")?;
    
    // Initialize session manager
    let session_manager = SessionManager::new(config.clone());

    match cli.command {
        // Default: Start listener with interface selection
        None => {
            // Interactive interface selection
            let mut selector = InterfaceSelector::new()?;
            let host = selector.select().await?;
            
            // Interactive port input
            let port = get_port_input()?;
            
            start_listener(host, port).await?;
        }
        
        Some(Commands::Listen { host, port }) => {
            // If host/port provided via CLI, skip interactive selection
            if host == "0.0.0.0" && port == 4444 {
                // Default values - show interactive selection
                let mut selector = InterfaceSelector::new()?;
                let selected_host = selector.select().await?;
                let selected_port = get_port_input()?;
                
                start_listener(selected_host, selected_port).await?;
            } else {
                // Explicit values - use directly
                start_listener(host, port).await?;
            }
        }
        
        Some(Commands::Sessions) => {
            handle_sessions_list().await?;
        }
        
        Some(Commands::Attach { id }) => {
            handle_session_attach(&id).await?;
        }
        
        Some(Commands::Kill { id }) => {
            handle_session_kill(&id).await?;
        }
        
        Some(Commands::Note { id, text }) => {
            handle_session_note(&id, text.join(" ")).await?;
        }
        
        Some(Commands::Modules) => {
            handle_list_modules();
        }
        Some(Commands::Wordlists { search }) => {
            handle_list_wordlists(search);
        }
        Some(Commands::Session { action }) => {
            handle_session_action(action, session_manager).await?;
        }
        Some(Commands::Shell { action }) => {
            handle_shell_action(action).await?;
        }
        Some(Commands::Module {
            name,
            target,
            wordlist,
            threads,
            verbose,
            status_codes,
            exclude_codes,
        }) => {
            handle_module_execution(
                name,
                target,
                wordlist,
                threads,
                verbose,
                status_codes,
                exclude_codes,
                &config,
                &session_manager,
            )
            .await?;
        }
        Some(Commands::Generate {
            module,
            target,
            output,
        }) => {
            handle_generate_payload(module, target, output, &config).await?;
        }
        Some(Commands::Web { action }) => {
            handle_web_action(action).await?;
        }
        Some(Commands::Scope { action }) => {
            handle_scope_action(action, &config).await?;
        }
        Some(Commands::Audit {
            session_id,
            export,
        }) => {
            handle_audit_command(session_id, export, &config).await?;
        }
        Some(Commands::Init { name }) => {
            handle_init_command(name).await?;
        }
    }

    Ok(())
}

// New simplified handlers for shell-first commands
async fn handle_sessions_list() -> Result<()> {
    use colored::Colorize;
    
    let state_file = "shell_sessions.json";
    
    println!();
    println!("{}", "Sessions".truecolor(37, 150, 190));
    println!();
    
    if std::path::Path::new(state_file).exists() {
        let content = tokio::fs::read_to_string(state_file).await?;
        if let Ok(sessions) = serde_json::from_str::<Vec<serde_json::Value>>(&content) {
            if sessions.is_empty() {
                println!("  {} none", "◦".truecolor(120, 120, 130));
            } else {
                for session in sessions {
                    let id = session["id"].as_str().unwrap_or("unknown");
                    let addr = session["remote_addr"].as_str().unwrap_or("unknown");
                    let hostname = session.get("hostname").and_then(|h| h.as_str());
                    let username = session.get("username").and_then(|u| u.as_str());
                    let privilege = session.get("privilege").and_then(|p| p.as_str()).unwrap_or("Unknown");
                    
                    let indicator = match session["state"].as_str() {
                        Some("Active") => "◉".truecolor(37, 150, 190),
                        Some("Background") => "◎".truecolor(86, 33, 213),
                        _ => "◦".truecolor(120, 120, 130),
                    };
                    
                    print!("  {} ", indicator);
                    
                    if let Some(host) = hostname {
                        print!("{}", host.truecolor(37, 150, 190));
                    } else {
                        print!("{}", addr.truecolor(37, 150, 190));
                    }
                    
                    if let Some(user) = username {
                        print!(" ({})", user.truecolor(86, 33, 213));
                    }
                    
                    if privilege == "Root" {
                        print!(" {}", "⚡".truecolor(255, 180, 80));
                    }
                    
                    println!(" {}", &id[..8].truecolor(120, 120, 130));
                }
            }
        } else {
            println!("  {} none", "◦".truecolor(120, 120, 130));
        }
    } else {
        println!("  {} none", "◦".truecolor(120, 120, 130));
    }
    
    println!();
    Ok(())
}

// Helper function to start listener
async fn start_listener(host: String, port: u16) -> Result<()> {
    let manager = Arc::new(ShellSessionManager::new());
    let listener = ShellListener::new(manager.clone());
    
    // Start listener in background
    let listen_host = host.clone();
    tokio::spawn(async move {
        if let Err(e) = listener.start_with_cleanup(&listen_host, port).await {
            eprintln!("[!] Shell listener error: {}", e);
        }
    });
    
    // Give listener time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    // Start interactive terminal
    let mut terminal = InteractiveTerminal::new(manager);
    terminal.run().await
}

async fn handle_session_attach(_id: &str) -> Result<()> {
    use colored::Colorize;
    println!();
    println!("{}", "Session attachment not yet implemented".truecolor(120, 120, 130));
    println!("{}", "Use 'cap listen' to start a new listener".truecolor(86, 33, 213));
    println!();
    Ok(())
}

async fn handle_session_kill(_id: &str) -> Result<()> {
    use colored::Colorize;
    println!();
    println!("{}", "Session termination not yet implemented".truecolor(120, 120, 130));
    println!();
    Ok(())
}

async fn handle_session_note(_id: &str, _note: String) -> Result<()> {
    use colored::Colorize;
    println!();
    println!("{}", "Session notes not yet implemented".truecolor(120, 120, 130));
    println!("{}", "Use Ctrl+N during an active session".truecolor(86, 33, 213));
    println!();
    Ok(())
}

async fn handle_session_action(action: SessionAction, manager: SessionManager) -> Result<()> {
    match action {
        SessionAction::List => {
            let sessions = manager.list_sessions().await;
            if sessions.is_empty() {
                println!("No active sessions.");
            } else {
                println!("\n{}", "Active Sessions:".bright_blue());
                for session in sessions {
                    println!(
                        "  {} - {} (Started: {})",
                        session.id.yellow(),
                        session.name.cyan(),
                        session.created_at
                    );
                }
            }
        }
        SessionAction::New { name } => {
            let session = manager.create_session(name).await?;
            println!(
                "\n{} Created session: {}",
                "✓".green(),
                session.id.yellow()
            );
        }
        SessionAction::Attach { id } => {
            println!(
                "\n{} Attaching to session: {}",
                "→".blue(),
                id.yellow()
            );
            // Interactive session logic would go here
        }
        SessionAction::Kill { id } => {
            manager.terminate_session(&id).await?;
            println!(
                "\n{} Terminated session: {}",
                "✗".red(),
                id.yellow()
            );
        }
    }
    Ok(())
}

fn short_id(id: &str) -> &str {
    if id.len() > 12 {
        &id[..12]
    } else {
        id
    }
}

async fn handle_shell_action(action: ShellAction) -> Result<()> {
    // Load shell session state from file
    let state_file = "shell_sessions.json";
    
    match action {
        ShellAction::List => {
            println!();
            println!("{}", "Shell Sessions:".bright_cyan());
            
            if std::path::Path::new(state_file).exists() {
                let content = tokio::fs::read_to_string(state_file).await?;
                if let Ok(sessions) = serde_json::from_str::<Vec<ShellSessionInfo>>(&content) {
                    if sessions.is_empty() {
                        println!("{}   (none)", "›".bright_black());
                        println!();
                        println!("{} Start a listener with: {}", "[*]".bright_black(), "cap listen".cyan());
                    } else {
                        for session in sessions {
                            let state_str = match session.state.as_str() {
                                "Active" => "ACTIVE".green(),
                                "Background" => "BACKGROUND".yellow(),
                                "Terminated" => "TERMINATED".red(),
                                _ => "UNKNOWN".white(),
                            };
                            
                            println!(
                                "{}   {} | {} | {}",
                                "›".bright_black(),
                                short_id(&session.id).bright_white(),
                                session.remote_addr.cyan(),
                                state_str
                            );
                        }
                    }
                } else {
                    println!("{}   (none)", "›".bright_black());
                }
            } else {
                println!("{}   (none)", "›".bright_black());
                println!();
                println!("{} Start a listener with: {}", "[*]".bright_black(), "cap listen".cyan());
            }
            println!();
        }
        ShellAction::Interact { id } => {
            println!();
            println!("{} To interact with shell sessions:", "[*]".bright_cyan());
            println!("{}   Run: {}", "›".bright_black(), "cap listen".cyan());
            println!();
        }
        ShellAction::Kill { id } => {
            println!(
                "\n{} Terminating shell session: {}",
                "✗".red(),
                short_id(&id).yellow()
            );
            // In the full implementation, this would signal the running listener
            println!("{} Use the interactive menu (F12) to manage sessions\n", "💡".to_string());
        }
        ShellAction::Upgrade { id } => {
            println!(
                "\n{} Shell upgrade (PTY) - Feature coming soon!",
                "🔧".yellow()
            );
            println!("  Session: {}", short_id(&id).yellow());
            println!("\n{} This will upgrade the shell to a full PTY with:", "💡".to_string());
            println!("  • Tab completion");
            println!("  • Command history");
            println!("  • Proper terminal size");
            println!("  • Interactive programs support\n");
        }
        ShellAction::Run { id, command } => {
            println!(
                "\n{} Running command on session {}:",
                "▶".green(),
                short_id(&id).yellow()
            );
            println!("  Command: {}\n", command.cyan());
            println!("{} Use interactive mode for real-time output\n", "💡".to_string());
        }
    }
    
    Ok(())
}

// Helper struct for shell session serialization
#[derive(serde::Serialize, serde::Deserialize)]
struct ShellSessionInfo {
    id: String,
    remote_addr: String,
    state: String,
    connected_at: String,
}

async fn handle_web_action(action: WebAction) -> Result<()> {
    let registry = web::ModuleRegistry::new();
    
    match action {
        WebAction::List => {
            registry.display_modules();
        }
        
        WebAction::Info { module } => {
            if let Some(web_module) = registry.get_module(&module) {
                web_module.display_info();
            } else {
                println!("\n{} Module not found: {}", "[!]".red(), module.red());
                println!("{} Use {} to see available modules\n", "[*]".bright_black(), "cap web list".cyan());
            }
        }
        
        WebAction::Run {
            module,
            request,
            injection_point,
            url,
            lhost,
            lport,
            dry_run,
            confirm_each,
        } => {
            if let Some(web_module) = registry.get_module(&module) {
                let operator = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());
                let mut context = web::ModuleContext::new(operator);
                
                // Load request file if provided
                if let Some(req_file) = request {
                    println!("{} Loading HTTP request from: {}", "[*]".bright_cyan(), req_file.bright_white());
                    
                    match web::request::HttpRequest::from_file(&req_file).await {
                        Ok(req) => {
                            println!("{} Request loaded successfully", "[+]".green());
                            println!("{}   Method: {} {}", "›".bright_black(), req.method, req.path);
                            
                            if let Some(host) = req.headers.get("Host") {
                                println!("{}   Host: {}", "›".bright_black(), host);
                            }
                            
                            // Show injection points
                            let points = req.injection_points();
                            println!("\n{} Available injection points:", "[*]".bright_cyan());
                            for (i, point) in points.iter().enumerate() {
                                println!("{}   [{}] {}", "›".bright_black(), i + 1, point.display());
                            }
                            println!();
                            
                            context.request = Some(req);
                        }
                        Err(e) => {
                            println!("{} Failed to load request: {}", "[!]".red(), e);
                            return Ok(());
                        }
                    }
                }
                
                // Set injection point if provided
                if let Some(point_name) = injection_point {
                    if let Some(ref req) = context.request {
                        // Try to find matching injection point
                        let points = req.injection_points();
                        let matching_point = points.iter().find(|p| {
                            match p {
                                web::request::InjectionPoint::QueryParam(n) |
                                web::request::InjectionPoint::BodyParam(n) |
                                web::request::InjectionPoint::Header(n) |
                                web::request::InjectionPoint::Cookie(n) => n == &point_name,
                            }
                        });
                        
                        if let Some(point) = matching_point {
                            println!("{} Injection point set: {}", "[+]".green(), point.display());
                            context.injection_point = Some(point.clone());
                        } else {
                            println!("{} Injection point '{}' not found in request", "[!]".yellow(), point_name);
                        }
                    }
                }
                
                // Set options
                if let Some(u) = url {
                    context.set_option("URL".to_string(), u);
                }
                
                if let Some(h) = lhost {
                    context.set_option("LHOST".to_string(), h);
                }
                
                if let Some(p) = lport {
                    context.set_option("LPORT".to_string(), p);
                }
                
                if dry_run {
                    context.set_option("DRY_RUN".to_string(), "true".to_string());
                    println!("{} DRY RUN MODE - No requests will be sent", "[*]".yellow());
                }
                
                if confirm_each {
                    context.set_option("CONFIRM_EACH".to_string(), "true".to_string());
                }
                
                // Execute module
                println!("{} Executing module: {}", "[*]".bright_cyan(), module.bright_white());
                println!();
                
                match web_module.execute(&context).await {
                    Ok(result) => {
                        println!("\n{}", "═".repeat(64).bright_black());
                        println!("{}", "EXECUTION RESULTS".bright_cyan().bold());
                        println!("{}", "─".repeat(64).bright_black());
                        
                        if result.success {
                            println!("{} Module executed successfully", "[+]".green());
                        } else {
                            println!("{} Module completed with no significant findings", "[*]".yellow());
                        }
                        
                        println!();
                        println!("{}:", "Findings".bright_yellow());
                        for finding in &result.findings {
                            println!("{}   {}", "›".bright_black(), finding);
                        }
                        
                        println!();
                        println!("Timestamp: {}", result.timestamp.format("%Y-%m-%d %H:%M:%S UTC").to_string().bright_black());
                        println!("{}", "═".repeat(64).bright_black());
                        println!();
                        
                        // TODO: Log to audit chain
                    }
                    Err(e) => {
                        println!("{} Module execution failed: {}", "[!]".red(), e);
                    }
                }
            } else {
                println!("\n{} Module not found: {}", "[!]".red(), module.red());
                println!("{} Use {} to see available modules\n", "[*]".bright_black(), "cap web list".cyan());
            }
        }
    }
    
    Ok(())
}

async fn handle_module_execution(
    name: String,
    target: String,
    wordlist: Option<String>,
    threads: Option<usize>,
    verbose: bool,
    status_codes: Option<Vec<u16>>,
    exclude_codes: Option<Vec<u16>>,
    config: &Config,
    session_manager: &SessionManager,
) -> Result<()> {
    use modules::ModuleExecutor;

    // Verify target is in scope
    if !config.scope.is_in_scope(&target) {
        anyhow::bail!(
            "Target '{}' is not in authorized scope. Add it with: cap scope add {}",
            target,
            target
        );
    }

    println!(
        "\n{} Executing module: {}",
        "→".blue(),
        name.cyan()
    );
    println!(
        "{} Target: {}",
        "→".blue(),
        target.yellow()
    );

    if verbose {
        println!("{} Verbose mode: enabled", "→".blue());
    }

    if let Some(ref codes) = status_codes {
        println!(
            "{} Status codes filter: {}",
            "→".blue(),
            codes.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(", ").yellow()
        );
    }

    if let Some(ref codes) = exclude_codes {
        println!(
            "{} Excluding status codes: {}",
            "→".blue(),
            codes.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(", ").yellow()
        );
    }

    let executor = ModuleExecutor::new(config.clone(), session_manager.clone());
    let results = executor
        .execute_with_options(
            &name,
            &target,
            wordlist,
            threads.unwrap_or(10),
            verbose,
            status_codes,
            exclude_codes,
        )
        .await?;

    println!(
        "\n{} Module execution completed",
        "✓".green()
    );
    println!("Results: {} findings", results.len());

    for result in results.iter().take(20) {
        println!("  • {}", result);
    }

    if results.len() > 20 {
        println!("  ... and {} more", results.len() - 20);
    }

    Ok(())
}

async fn handle_scope_action(action: ScopeAction, config: &Config) -> Result<()> {
    match action {
        ScopeAction::Add { target } => {
            config.scope.add_target(&target)?;
            // Save config to persist scope changes
            config.save("config/default.toml")?;
            println!(
                "\n{} Added target to scope: {}",
                "✓".green(),
                target.yellow()
            );
            println!("{} Scope saved to config/default.toml", "💾".to_string());
        }
        ScopeAction::Remove { target } => {
            config.scope.remove_target(&target)?;
            // Save config to persist scope changes
            config.save("config/default.toml")?;
            println!(
                "\n{} Removed target from scope: {}",
                "✗".red(),
                target.yellow()
            );
            println!("{} Scope saved to config/default.toml", "💾".to_string());
        }
        ScopeAction::List => {
            let targets = config.scope.list_targets();
            if targets.is_empty() {
                println!("\nNo targets in scope. Add targets with: cap scope add <target>");
            } else {
                println!("\n{}", "Authorized Targets:".bright_blue());
                for target in targets {
                    println!("  • {}", target.yellow());
                }
            }
        }
        ScopeAction::Check { target } => {
            let in_scope = config.scope.is_in_scope(&target);
            if in_scope {
                println!(
                    "\n{} Target '{}' is in authorized scope",
                    "✓".green(),
                    target.yellow()
                );
            } else {
                println!(
                    "\n{} Target '{}' is NOT in authorized scope",
                    "✗".red(),
                    target.yellow()
                );
            }
        }
    }
    Ok(())
}

async fn handle_audit_command(
    session_id: Option<String>,
    export: Option<String>,
    config: &Config,
) -> Result<()> {
    use core::audit::AuditLogger;

    let logger = AuditLogger::new(&config.audit.log_path)?;
    let logs = logger.read_logs(session_id.as_deref())?;

    if let Some(export_path) = export {
        logger.export_logs(&logs, &export_path)?;
        println!(
            "\n{} Exported {} audit logs to: {}",
            "✓".green(),
            logs.len(),
            export_path.yellow()
        );
    } else {
        println!("\n{}", "Audit Logs:".bright_blue());
        for log in logs.iter().take(50) {
            println!(
                "  [{}] {} - {}",
                log.timestamp,
                log.event_type.cyan(),
                log.description
            );
        }
        if logs.len() > 50 {
            println!("  ... and {} more logs", logs.len() - 50);
        }
    }

    Ok(())
}

async fn handle_init_command(name: String) -> Result<()> {
    use std::fs;

    println!(
        "\n{} Initializing new CAP project: {}",
        "→".blue(),
        name.cyan()
    );

    fs::create_dir_all(&name)?;
    fs::create_dir_all(format!("{}/config", name))?;
    fs::create_dir_all(format!("{}/wordlists", name))?;
    fs::create_dir_all(format!("{}/logs", name))?;
    fs::create_dir_all(format!("{}/reports", name))?;

    let default_config = r#"[general]
name = "CAP Project"
description = "Security assessment project"

[server]
host = "127.0.0.1"
port = 8443
tls_enabled = false

[scope]
authorized_targets = []

[audit]
log_path = "logs/audit.jsonl"
retention_days = 90

[modules]
default_threads = 10
timeout_seconds = 300
"#;

    fs::write(format!("{}/config/default.toml", name), default_config)?;

    println!(
        "{} Project structure created successfully",
        "✓".green()
    );
    println!("\nNext steps:");
    println!("  1. cd {}", name);
    println!("  2. cap scope add <target>");
    println!("  3. cap listen");

    Ok(())
}

fn handle_list_modules() {
    println!("\n{}", "═══════════════════════════════════════════════════════════════".bright_blue());
    println!("{}", "                    CAP SECURITY MODULES".bright_yellow().bold());
    println!("{}", "═══════════════════════════════════════════════════════════════\n".bright_blue());

    let modules = vec![
        (
            "web-enum",
            "Web Application Enumeration",
            "Discovers hidden directories, files, and endpoints on web servers using\nwordlist-based enumeration. Identifies admin panels, API endpoints,\nconfiguration files, and backup directories.",
            vec!["Directory discovery", "File enumeration", "API endpoint detection", "Status code filtering"],
        ),
        (
            "dns-enum",
            "DNS & Subdomain Enumeration",
            "Performs DNS resolution to discover subdomains and map an organization's\nexternal attack surface. Useful for reconnaissance and asset discovery.",
            vec!["Subdomain discovery", "DNS resolution", "IP mapping", "IPv4/IPv6 support"],
        ),
        (
            "port-scan",
            "Network Port Scanning",
            "Identifies open TCP ports and running services on target systems.\nScans common ports to discover network services and potential entry points.",
            vec!["Port scanning", "Service detection", "TCP connect", "Common ports (21-27017)"],
        ),
    ];

    for (i, (name, title, description, features)) in modules.iter().enumerate() {
        println!("{} {}", format!("{}.", i + 1).bright_black(), title.bright_cyan().bold());
        println!("   {} {}", "Module ID:".bright_black(), name.yellow());
        println!();
        println!("   {}", description.white());
        println!();
        println!("   {}:", "Features".bright_black());
        for feature in features {
            println!("     {} {}", "•".green(), feature);
        }
        println!();
        
        println!("   {}:", "Usage".bright_black());
        println!("     {}", format!("cap module --name {} --target <target>", name).bright_white());
        println!("     {}", format!("cap generate --module {} --target <target>", name).bright_white());
        println!();
        
        if i < modules.len() - 1 {
            println!("{}", "───────────────────────────────────────────────────────────────".bright_black());
            println!();
        }
    }

    println!("{}", "═══════════════════════════════════════════════════════════════".bright_blue());
    println!("\n{} Use {} to see all available commands", "💡".to_string(), "cap --help".yellow());
    println!("{} Use {} to generate a payload for a module\n", "🔧".to_string(), "cap generate --module <name> --target <target>".yellow());
}

fn handle_list_wordlists(search: Option<String>) {
    use modules::web_enum::WebEnumerationModule;
    use std::collections::HashMap;

    println!("\n{}", "═══════════════════════════════════════════════════════════════".bright_blue());
    println!("{}", "                    AVAILABLE WORDLISTS".bright_yellow().bold());
    println!("{}", "═══════════════════════════════════════════════════════════════\n".bright_blue());

    let wordlists = WebEnumerationModule::discover_wordlists();

    if wordlists.is_empty() {
        println!("{} No wordlists found in standard locations", "⚠".yellow());
        println!("\nSearched locations:");
        println!("  • /usr/share/wordlists/");
        println!("  • /usr/share/seclists/");
        println!("  • /snap/seclists/current/");
        println!("  • wordlists/");
        println!("\n{} Install SecLists: sudo apt install seclists", "💡".to_string());
        return;
    }

    // Group by directory
    let mut by_directory: HashMap<String, Vec<&std::path::PathBuf>> = HashMap::new();
    for wl in &wordlists {
        if let Some(parent) = wl.parent() {
            let dir = parent.to_string_lossy().to_string();
            by_directory.entry(dir).or_insert_with(Vec::new).push(wl);
        }
    }

    // Filter by search term if provided
    let filtered_wordlists: Vec<_> = if let Some(ref search_term) = search {
        wordlists.iter().filter(|wl| {
            wl.to_string_lossy().to_lowercase().contains(&search_term.to_lowercase())
        }).collect()
    } else {
        wordlists.iter().collect()
    };

    if search.is_some() && filtered_wordlists.is_empty() {
        println!("{} No wordlists found matching: {}", "⚠".yellow(), search.unwrap().yellow());
        return;
    }

    println!("{} Found {} wordlists\n", "✓".green(), filtered_wordlists.len().to_string().yellow());

    // Display grouped wordlists
    let mut dirs: Vec<_> = by_directory.keys().collect();
    dirs.sort();

    for dir in dirs {
        if let Some(lists) = by_directory.get(dir) {
            // Filter lists for this directory
            let filtered_in_dir: Vec<_> = lists.iter()
                .filter(|wl| {
                    if let Some(ref st) = search {
                        wl.to_string_lossy().to_lowercase().contains(&st.to_lowercase())
                    } else {
                        true
                    }
                })
                .collect();

            if filtered_in_dir.is_empty() {
                continue;
            }

            println!("{}", dir.bright_cyan().bold());
            println!("{}", "─".repeat(60).bright_black());

            for wl in filtered_in_dir {
                if let Some(filename) = wl.file_name() {
                    let name = filename.to_string_lossy();
                    
                    // Get file size
                    let size = if let Ok(metadata) = std::fs::metadata(wl) {
                        let bytes = metadata.len();
                        if bytes < 1024 {
                            format!("{} B", bytes)
                        } else if bytes < 1024 * 1024 {
                            format!("{:.1} KB", bytes as f64 / 1024.0)
                        } else {
                            format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
                        }
                    } else {
                        "unknown".to_string()
                    };

                    // Count lines
                    let lines = if let Ok(file) = std::fs::File::open(wl) {
                        std::io::BufRead::lines(std::io::BufReader::new(file)).count()
                    } else {
                        0
                    };

                    println!("  {} {} {} {}",
                        "•".green(),
                        name.bright_white(),
                        format!("[{} lines]", lines).bright_black(),
                        format!("({})", size).bright_black()
                    );
                }
            }
            println!();
        }
    }

    println!("{}", "═══════════════════════════════════════════════════════════════".bright_blue());
    println!("\n{} Usage:", "💡".to_string());
    println!("  {} Search wordlists", "cap wordlists --search directory".bright_white());
    println!("  {} Use in web-enum", "cap module --name web-enum --target <url> --wordlist <path>".bright_white());
    println!("\n{} Common wordlists:", "📋".to_string());
    println!("  • common.txt - Basic directory/file names");
    println!("  • directory-list-2.3-small.txt - Popular directories (87K lines)");
    println!("  • raft-small-words.txt - Compact wordlist for quick scans");
    println!("  • big.txt - Comprehensive wordlist (20K+ lines)\n");
}

async fn handle_generate_payload(
    module: String,
    target: String,
    output: Option<String>,
    config: &Config,
) -> Result<()> {
    use std::fs::File;
    use std::io::Write;

    // Verify target is in scope
    if !config.scope.is_in_scope(&target) {
        anyhow::bail!(
            "Target '{}' is not in authorized scope. Add it with: cap scope add {}",
            target,
            target
        );
    }

    println!("\n{} Generating payload for module: {}", "🔧".to_string(), module.cyan());
    println!("{} Target: {}\n", "→".blue(), target.yellow());

    let payload = match module.as_str() {
        "web-enum" => {
            format!(
                r#"{{
  "type": "web-enumeration",
  "module": "web-enum",
  "target": "{}",
  "config": {{
    "wordlist": "wordlists/common.txt",
    "threads": 10,
    "timeout": 300,
    "methods": ["GET"],
    "follow_redirects": true,
    "status_codes": [200, 403, 401, 500]
  }},
  "metadata": {{
    "generated_at": "{}",
    "operator": "{}",
    "description": "Web application enumeration task"
  }}
}}"#,
                target,
                chrono::Utc::now().to_rfc3339(),
                whoami::username()
            )
        }
        "dns-enum" => {
            format!(
                r#"{{
  "type": "dns-enumeration",
  "module": "dns-enum",
  "target": "{}",
  "config": {{
    "wordlist": "wordlists/subdomains.txt",
    "threads": 50,
    "timeout": 300,
    "resolve_ipv4": true,
    "resolve_ipv6": true,
    "recursive": false
  }},
  "metadata": {{
    "generated_at": "{}",
    "operator": "{}",
    "description": "DNS and subdomain enumeration task"
  }}
}}"#,
                target,
                chrono::Utc::now().to_rfc3339(),
                whoami::username()
            )
        }
        "port-scan" => {
            format!(
                r#"{{
  "type": "port-scan",
  "module": "port-scan",
  "target": "{}",
  "config": {{
    "ports": "common",
    "threads": 100,
    "timeout": 2,
    "scan_type": "tcp-connect",
    "service_detection": true
  }},
  "metadata": {{
    "generated_at": "{}",
    "operator": "{}",
    "description": "Network port scanning task"
  }}
}}"#,
                target,
                chrono::Utc::now().to_rfc3339(),
                whoami::username()
            )
        }
        _ => {
            anyhow::bail!("Unknown module: {}. Use 'cap modules' to list available modules.", module);
        }
    };

    // Display payload
    println!("{}", "Generated Payload:".bright_green().bold());
    println!("{}", "─────────────────────────────────────────────────────────".bright_black());
    println!("{}", payload.bright_white());
    println!("{}", "─────────────────────────────────────────────────────────".bright_black());

    // Save to file if output specified
    if let Some(output_path) = output {
        let mut file = File::create(&output_path)?;
        file.write_all(payload.as_bytes())?;
        println!("\n{} Payload saved to: {}", "✓".green(), output_path.yellow());
    } else {
        println!("\n{} Use {} to save payload to file", "💡".to_string(), "--output <file>".yellow());
    }

    println!("\n{}", "Usage:".bright_blue());
    println!("  Execute immediately: {}", format!("cap module --name {} --target {}", module, target).bright_white());
    println!("  Via API: {}", "POST /api/modules/execute with payload".bright_white());

    Ok(())
}


