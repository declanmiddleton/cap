use anyhow::Result;
use colored::Colorize;
use crossterm::{
    event::{self, poll, read, Event, KeyCode, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use std::io::{self, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::time::{interval, Duration};
use tracing::{error, info};

use super::session::ShellSessionManager;

pub struct InteractiveTerminal {
    manager: Arc<ShellSessionManager>,
    should_stop_listener: Arc<AtomicBool>,
}

impl InteractiveTerminal {
    pub fn new(manager: Arc<ShellSessionManager>) -> Self {
        Self {
            manager,
            should_stop_listener: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        // Print welcome header
        self.print_header();
        
        // Enable raw mode for keyboard input
        enable_raw_mode()?;
        
        let result = self.run_loop().await;
        
        // Restore terminal
        disable_raw_mode()?;
        
        result
    }

    fn print_header(&self) {
        println!();
        println!("{}", "[+] Listening for reverse shells...".green());
        println!();
        println!("{}  {} to detach (listener keeps running)", "›".bright_black(), "F12 or Ctrl+D".bright_white());
        println!("{}  {} to stop listener and exit", "›".bright_black(), "Ctrl+Q or q".bright_white());
        println!("{}  {} to list active sessions", "›".bright_black(), "Ctrl+L or l".bright_white());
        println!("{}  {} for help", "›".bright_black(), "Ctrl+H or h".bright_white());
        println!();
    }

    async fn run_loop(&mut self) -> Result<()> {
        let mut output_interval = interval(Duration::from_millis(100));
        let mut last_session_count = 0;

        loop {
            tokio::select! {
                _ = output_interval.tick() => {
                    // Check for new sessions
                    let session_count = self.manager.session_count();
                    if session_count != last_session_count {
                        last_session_count = session_count;
                        self.print_session_update().await;
                    }

                    // Print output from active session
                    if let Some(session) = self.manager.get_active_session().await {
                        let mut output_rx = session.output_rx.write().await;
                        while let Ok(line) = output_rx.try_recv() {
                            print!("{}", line);
                            io::stdout().flush().unwrap();
                        }
                    }
                }

                _ = async {
                    if poll(Duration::from_millis(50)).unwrap_or(false) {
                        match read() {
                            Ok(Event::Key(key)) => {
                                if let Err(e) = self.handle_key_event(key).await {
                                    error!("Error handling key event: {}", e);
                                }
                            }
                            _ => {}
                        }
                    }
                } => {}
            }

            // Check if we should stop listener
            if self.should_stop_listener.load(Ordering::Relaxed) {
                println!("\n{}", "[!] Stopping listener...".yellow());
                break;
            }
        }

        Ok(())
    }

    async fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match key.code {
            // F12 or Ctrl+D - Detach (exit but keep listener running)
            KeyCode::F(12) => {
                println!("\n{}", "[*] Detaching from listener (continues in background)".bright_black());
                println!("{} Use {} to see sessions\n", "[*]".bright_black(), "cap shell list".cyan());
                std::process::exit(0);
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                println!("\n{}", "[*] Detaching from listener (continues in background)".bright_black());
                println!("{} Use {} to see sessions\n", "[*]".bright_black(), "cap shell list".cyan());
                std::process::exit(0);
            }
            // Ctrl+Q or 'q' - Stop listener completely
            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_stop_listener.store(true, Ordering::Relaxed);
            }
            KeyCode::Char('q') => {
                self.should_stop_listener.store(true, Ordering::Relaxed);
            }
            // Ctrl+L or 'l' - List sessions
            KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.list_sessions().await;
            }
            KeyCode::Char('l') => {
                self.list_sessions().await;
            }
            // Ctrl+H or 'h' - Show help
            KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.show_help();
            }
            KeyCode::Char('h') => {
                self.show_help();
            }
            // Ctrl+C - Force exit
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                println!("\n{}", "[!] Exiting...".yellow());
                std::process::exit(0);
            }
            _ => {}
        }
        Ok(())
    }

    async fn print_session_update(&self) {
        let sessions = self.manager.list_sessions();
        if sessions.is_empty() {
            return;
        }

        println!("\n{} {} session(s) active", "[+]".green(), sessions.len());
        for (id, state) in sessions {
            let state_str = match state {
                super::session::ShellState::Active => "ACTIVE".green(),
                super::session::ShellState::Background => "BACKGROUND".yellow(),
                super::session::ShellState::Terminated => "TERMINATED".red(),
            };
            
            if let Some(session) = self.manager.get_session(&id) {
                println!(
                    "{}   {} | {} | {}",
                    "›".bright_black(),
                    &id[..8].bright_white(),
                    session.metadata.remote_addr.cyan(),
                    state_str
                );
            }
        }
        println!();
    }

    async fn list_sessions(&self) {
        let sessions = self.manager.list_sessions();
        
        println!();
        println!("{} Active sessions:", "[*]".bright_cyan());
        
        if sessions.is_empty() {
            println!("{}   (none)", "›".bright_black());
        } else {
            for (id, state) in sessions {
                let state_str = match state {
                    super::session::ShellState::Active => "ACTIVE".green(),
                    super::session::ShellState::Background => "BACKGROUND".yellow(),
                    super::session::ShellState::Terminated => "TERMINATED".red(),
                };
                
                if let Some(session) = self.manager.get_session(&id) {
                    println!(
                        "{}   {} | {} | {}",
                        "›".bright_black(),
                        &id[..8].bright_white(),
                        session.metadata.remote_addr.cyan(),
                        state_str
                    );
                }
            }
        }
        println!();
    }

    fn show_help(&self) {
        println!();
        println!("{} Keyboard shortcuts:", "[*]".bright_cyan());
        println!("{}   {} - Detach (listener keeps running)", "›".bright_black(), "F12 or Ctrl+D".bright_white());
        println!("{}   {} - Stop listener", "›".bright_black(), "Ctrl+Q or q".bright_white());
        println!("{}   {} - List sessions", "›".bright_black(), "Ctrl+L or l".bright_white());
        println!("{}   {} - Show help", "›".bright_black(), "Ctrl+H or h".bright_white());
        println!();
    }
}

