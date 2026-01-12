use anyhow::Result;
use chrono::Utc;
use colored::*;
use crossterm::{
    cursor,
    event::{self, poll, read, Event, KeyCode, KeyModifiers},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
};
use std::io::{self, Write};
use std::sync::Arc;
use std::time::Duration as StdDuration;
use tokio::time::{interval, Duration};
use tracing::{error, info};

use super::session::{ShellSessionManager, PrivilegeLevel};

// Modern color scheme
const PRIMARY_COLOR: Color = Color::Rgb { r: 37, g: 150, b: 190 };    // #2596be
const SECONDARY_COLOR: Color = Color::Rgb { r: 86, g: 33, b: 213 };   // #5621d5
const ACCENT_COLOR: Color = Color::Rgb { r: 100, g: 180, b: 200 };
const MUTED_COLOR: Color = Color::Rgb { r: 120, g: 120, b: 130 };
const SUCCESS_COLOR: Color = Color::Rgb { r: 80, g: 200, b: 120 };
const WARNING_COLOR: Color = Color::Rgb { r: 255, g: 180, b: 80 };

pub struct InteractiveTerminal {
    manager: Arc<ShellSessionManager>,
    should_exit: bool,
    animation_frame: u8,
}

impl InteractiveTerminal {
    pub fn new(manager: Arc<ShellSessionManager>) -> Self {
        Self {
            manager,
            should_exit: false,
            animation_frame: 0,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        // Clear screen and show welcome
        self.clear_screen()?;
        self.show_welcome_animation().await?;
        
        // Enable raw mode for direct keyboard input
        enable_raw_mode()?;
        
        let result = self.run_loop().await;
        
        // Restore terminal
        disable_raw_mode()?;
        println!();
        
        result
    }

    fn clear_screen(&self) -> Result<()> {
        execute!(io::stdout(), Clear(ClearType::All), cursor::MoveTo(0, 0))?;
        Ok(())
    }

    async fn show_welcome_animation(&mut self) -> Result<()> {
        // Brief glow-in effect - fast and minimal
        for intensity in [0.4, 0.7, 1.0] {
            print!("\r");
            let color = if intensity < 1.0 {
                Color::Rgb { 
                    r: (37.0 * intensity) as u8, 
                    g: (150.0 * intensity) as u8, 
                    b: (190.0 * intensity) as u8 
                }
            } else {
                PRIMARY_COLOR
            };
            
            self.print_colored("◉ ", color);
            print!("Listening");
            
            io::stdout().flush()?;
            tokio::time::sleep(Duration::from_millis(60)).await;
        }
        
        println!("\n");
        Ok(())
    }

    fn print_colored(&self, text: &str, color: Color) {
        let _ = execute!(io::stdout(), SetForegroundColor(color), Print(text), ResetColor);
    }

    async fn run_loop(&mut self) -> Result<()> {
        let mut output_interval = interval(Duration::from_millis(50));
        let mut animation_interval = interval(Duration::from_millis(200));
        let mut last_session_count = 0;
        let mut in_session = false;
        let mut current_input = String::new();

        loop {
            tokio::select! {
                _ = output_interval.tick() => {
                    // Check for new sessions
                    let session_count = self.manager.session_count();
                    if session_count != last_session_count {
                        if session_count > last_session_count {
                            // New session - pulse animation
                            self.show_session_capture_animation().await?;
                        }
                        last_session_count = session_count;
                    }

                    // Print output from active session
                    if let Some(session) = self.manager.get_active_session().await {
                        let state = session.get_state().await;
                        
                        // Check if session is disconnected but being kept alive
                        if state == super::session::ShellState::Background {
                            if in_session {
                                self.show_reconnection_shimmer().await?;
                            }
                        } else if !in_session {
                            in_session = true;
                            self.show_session_connected_animation().await?;
                        }
                        
                        let mut output_rx = session.output_rx.write().await;
                        while let Ok(line) = output_rx.try_recv() {
                            print!("{}", line);
                            io::stdout().flush()?;
                        }
                        
                        // Show persistent prompt with session context only if active
                        if state == super::session::ShellState::Active {
                            self.show_session_prompt(&session).await?;
                        }
                    } else if in_session {
                        in_session = false;
                        println!();
                        self.print_colored("◦ ", MUTED_COLOR);
                        println!("Waiting for connection...");
                    }
                }

                _ = animation_interval.tick() => {
                    self.animation_frame = self.animation_frame.wrapping_add(1);
                }

                _ = async {
                    if poll(Duration::from_millis(10)).unwrap_or(false) {
                        match read() {
                            Ok(Event::Key(key)) => {
                                if let Err(e) = self.handle_key_event(key, &mut current_input).await {
                                    error!("Error handling key event: {}", e);
                                }
                            }
                            _ => {}
                        }
                    }
                } => {}
            }

            if self.should_exit {
                break;
            }

            // Cleanup terminated sessions silently
            self.manager.cleanup_terminated_sessions().await;
        }

        self.show_exit_animation().await?;
        Ok(())
    }

    async fn show_session_capture_animation(&self) -> Result<()> {
        // Brief glow-in effect when session captured
        println!();
        
        for intensity in [0.5, 0.8, 1.0, 0.9, 1.0] {
            print!("\r");
            
            let color = Color::Rgb { 
                r: (80.0 + 20.0 * intensity) as u8, 
                g: (180.0 + 20.0 * intensity) as u8, 
                b: (120.0 * intensity) as u8 
            };
            
            self.print_colored("◉ ", color);
            print!("Session captured");
            io::stdout().flush()?;
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        
        println!("\n");
        Ok(())
    }

    async fn show_session_connected_animation(&self) -> Result<()> {
        // Soft horizontal sweep under prompt
        for width in 1..=25 {
            print!("\r");
            for _ in 0..width {
                self.print_colored("▔", PRIMARY_COLOR);
            }
            io::stdout().flush()?;
            tokio::time::sleep(Duration::from_millis(3)).await;
        }
        println!("\n");
        Ok(())
    }

    async fn show_exit_animation(&self) -> Result<()> {
        // Smooth fade-out
        println!();
        for intensity in [1.0, 0.6, 0.3, 0.1] {
            print!("\r");
            
            let color = Color::Rgb { 
                r: (37.0 * intensity) as u8, 
                g: (150.0 * intensity) as u8, 
                b: (190.0 * intensity) as u8 
            };
            
            let symbol = if intensity > 0.5 { "◉" } else { "◦" };
            self.print_colored(symbol, color);
            print!(" ");
            
            io::stdout().flush()?;
            tokio::time::sleep(Duration::from_millis(60)).await;
        }
        println!("\r                    ");
        Ok(())
    }

    async fn show_session_transition(&self) -> Result<()> {
        // Smooth fade when switching sessions
        for intensity in [1.0, 0.7, 0.4, 0.7, 1.0] {
            print!("\r");
            
            let color = Color::Rgb { 
                r: (37.0 * intensity) as u8, 
                g: (150.0 * intensity) as u8, 
                b: (190.0 * intensity) as u8 
            };
            
            self.print_colored("◉ ", color);
            print!("Switching");
            io::stdout().flush()?;
            tokio::time::sleep(Duration::from_millis(40)).await;
        }
        
        print!("\r                    \r");
        Ok(())
    }

    async fn show_reconnection_shimmer(&self) -> Result<()> {
        // Low-frequency pulse during reconnection attempts
        let pulse_chars = ['◉', '◎', '◉', '◉'];
        let frame = (self.animation_frame % 4) as usize;
        
        print!("\r");
        self.print_colored(&format!("{} ", pulse_chars[frame]), SECONDARY_COLOR);
        print!("Connection interrupted");
        io::stdout().flush()?;
        
        Ok(())
    }

    async fn show_session_prompt(&self, session: &Arc<super::session::ShellSession>) -> Result<()> {
        let metadata = session.get_metadata().await;
        
        // Calculate session age
        let age = Utc::now().signed_duration_since(metadata.connected_at);
        let age_str = if age.num_hours() > 0 {
            format!("{}h", age.num_hours())
        } else if age.num_minutes() > 0 {
            format!("{}m", age.num_minutes())
        } else {
            format!("{}s", age.num_seconds())
        };

        // Build prompt with metadata
        print!("\r");
        
        // Primary indicator
        self.print_colored("◉ ", PRIMARY_COLOR);
        
        // Target identity
        if let Some(hostname) = &metadata.hostname {
            self.print_colored(hostname, PRIMARY_COLOR);
        } else {
            self.print_colored(&metadata.remote_addr, PRIMARY_COLOR);
        }
        
        // Privilege level
        let priv_symbol = match metadata.privilege {
            PrivilegeLevel::Root => {
                self.print_colored(" ⚡", WARNING_COLOR);
                " root"
            }
            PrivilegeLevel::User => " user",
            PrivilegeLevel::Unknown => "",
        };
        
        if !priv_symbol.is_empty() {
            self.print_colored(priv_symbol, SECONDARY_COLOR);
        }
        
        // Session age
        print!(" ");
        self.print_colored(&format!("({})", age_str), MUTED_COLOR);
        
        // Operator notes if set
        if let Some(notes) = &metadata.operator_notes {
            print!(" ");
            self.print_colored(&format!("[{}]", notes), ACCENT_COLOR);
        }
        
        print!(" ");
        io::stdout().flush()?;
        
        Ok(())
    }

    async fn handle_key_event(&mut self, key: crossterm::event::KeyEvent, input: &mut String) -> Result<()> {
        if let Some(session) = self.manager.get_active_session().await {
            match key.code {
                KeyCode::Enter => {
                    println!();
                    let command = format!("{}\n", input);
                    session.send_command(command).await?;
                    input.clear();
                }
                KeyCode::Char(c) => {
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        match c {
                            'd' => {
                                // Detach
                                println!();
                                self.print_colored("◦ ", MUTED_COLOR);
                                println!("Detached");
                                self.should_exit = true;
                            }
                            'c' => {
                                // Send Ctrl+C to shell
                                session.send_command("\x03".to_string()).await?;
                            }
                            'l' => {
                                // List sessions
                                self.list_sessions().await?;
                            }
                            'n' => {
                                // Add note
                                self.add_session_note(&session).await?;
                            }
                            _ => {}
                        }
                    } else {
                        input.push(c);
                        print!("{}", c);
                        io::stdout().flush()?;
                    }
                }
                KeyCode::Backspace => {
                    if !input.is_empty() {
                        input.pop();
                        print!("\x08 \x08");
                        io::stdout().flush()?;
                    }
                }
                KeyCode::Esc => {
                    // Background session
                    println!();
                    self.print_colored("◦ ", MUTED_COLOR);
                    println!("Session backgrounded");
                    self.manager.background_session(&session.id).await?;
                }
                _ => {}
            }
        } else {
            // No active session - handle commands
            match key.code {
                KeyCode::Char('q') | KeyCode::Char('Q') => {
                    self.should_exit = true;
                }
                KeyCode::Char('l') | KeyCode::Char('L') => {
                    self.list_sessions().await?;
                }
                KeyCode::Char('h') | KeyCode::Char('H') => {
                    self.show_help().await?;
                }
                KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'c' => {
                    self.should_exit = true;
                }
                KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'd' => {
                    self.should_exit = true;
                }
                _ => {}
            }
        }
        
        Ok(())
    }

    async fn list_sessions(&self) -> Result<()> {
        println!("\n");
        self.print_colored("Sessions\n", PRIMARY_COLOR);
        
        let sessions = self.manager.list_sessions();
        
        if sessions.is_empty() {
            self.print_colored("  ◦ ", MUTED_COLOR);
            println!("none");
        } else {
            for (id, state) in sessions {
                if let Some(session) = self.manager.get_session(&id) {
                    let metadata = session.get_metadata().await;
                    
                    // State indicator
                    let (indicator, color) = match state {
                        super::session::ShellState::Active => ("◉", PRIMARY_COLOR),
                        super::session::ShellState::Background => ("◎", SECONDARY_COLOR),
                        super::session::ShellState::Terminated => ("◦", MUTED_COLOR),
                    };
                    
                    print!("  ");
                    self.print_colored(indicator, color);
                    print!(" ");
                    
                    // Session info
                    if let Some(hostname) = &metadata.hostname {
                        self.print_colored(hostname, PRIMARY_COLOR);
                    } else {
                        self.print_colored(&metadata.remote_addr, PRIMARY_COLOR);
                    }
                    
                    if let Some(username) = &metadata.username {
                        print!(" ");
                        self.print_colored(&format!("({})", username), SECONDARY_COLOR);
                    }
                    
                    // Privilege
                    match metadata.privilege {
                        PrivilegeLevel::Root => {
                            print!(" ");
                            self.print_colored("⚡", WARNING_COLOR);
                        }
                        _ => {}
                    }
                    
                    // Age
                    let age = Utc::now().signed_duration_since(metadata.connected_at);
                    let age_str = if age.num_hours() > 0 {
                        format!("{}h", age.num_hours())
                    } else if age.num_minutes() > 0 {
                        format!("{}m", age.num_minutes())
                    } else {
                        format!("{}s", age.num_seconds())
                    };
                    
                    print!(" ");
                    self.print_colored(&age_str, MUTED_COLOR);
                    
                    println!();
                }
            }
        }
        
        println!();
        Ok(())
    }

    async fn show_help(&self) -> Result<()> {
        println!("\n");
        self.print_colored("Controls\n", PRIMARY_COLOR);
        println!();
        
        self.print_colored("  Ctrl+D   ", SECONDARY_COLOR);
        println!("detach");
        
        self.print_colored("  Ctrl+C   ", SECONDARY_COLOR);
        println!("interrupt");
        
        self.print_colored("  Ctrl+L   ", SECONDARY_COLOR);
        println!("sessions");
        
        self.print_colored("  Ctrl+N   ", SECONDARY_COLOR);
        println!("note");
        
        self.print_colored("  Esc      ", SECONDARY_COLOR);
        println!("background");
        
        println!();
        Ok(())
    }

    async fn add_session_note(&self, session: &Arc<super::session::ShellSession>) -> Result<()> {
        println!();
        self.print_colored("◉ ", ACCENT_COLOR);
        print!("Note: ");
        io::stdout().flush()?;
        
        // Simple note input
        let mut note = String::new();
        disable_raw_mode()?;
        io::stdin().read_line(&mut note)?;
        enable_raw_mode()?;
        
        let note = note.trim().to_string();
        if !note.is_empty() {
            session.set_notes(note).await;
            print!("\r");
            self.print_colored("◉ ", SUCCESS_COLOR);
            println!("Saved\n");
        } else {
            print!("\r                    \r");
        }
        
        Ok(())
    }
}
