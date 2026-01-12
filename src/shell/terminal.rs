use anyhow::Result;
use colored::*;
use crossterm::{
    cursor, execute,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{disable_raw_mode, enable_raw_mode, size, Clear, ClearType},
};
use std::io::{self, Write};
use std::sync::Arc;
use std::time::Duration;

use super::session::ShellSessionManager;
use super::menu::MainMenu;
use super::formatting::{get_safe_width, horizontal_line};

const PRIMARY_COLOR: Color = Color::Rgb { r: 37, g: 150, b: 190 };
const SECONDARY_COLOR: Color = Color::Rgb { r: 86, g: 33, b: 213 };
const SUCCESS_COLOR: Color = Color::Rgb { r: 80, g: 200, b: 120 };
const WARNING_COLOR: Color = Color::Rgb { r: 255, g: 180, b: 80 };
const MUTED_COLOR: Color = Color::Rgb { r: 120, g: 120, b: 130 };

pub enum InteractionResult {
    Detached,      // User pressed Esc to return to menu
    SessionEnded,  // Session terminated
}

#[derive(PartialEq, Eq)]
enum TerminalState {
    Listening,      // Waiting for connections
    InMenu,         // Menu is displayed
    InShell,        // Interacting with shell
}

pub struct InteractiveTerminal {
    manager: Arc<ShellSessionManager>,
    state: TerminalState,
}

impl InteractiveTerminal {
    pub fn new(manager: Arc<ShellSessionManager>) -> Self {
        Self { 
            manager,
            state: TerminalState::Listening,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        // Initial state: listening
        self.show_welcome_banner()?;
        self.state = TerminalState::Listening;
        
        loop {
            match self.state {
                TerminalState::Listening => {
                    // Wait for active session or user requesting menu
                    if let Some(session) = self.wait_for_session().await? {
                        // Got an active session - enter shell mode
                        self.state = TerminalState::InShell;
                        
                        match self.enter_interactive_mode(session).await? {
                            InteractionResult::Detached => {
                                // User wants menu
                                self.state = TerminalState::InMenu;
                            }
                            InteractionResult::SessionEnded => {
                                // Session ended - back to listening
                                self.show_welcome_banner()?;
                                self.state = TerminalState::Listening;
                            }
                        }
                    } else {
                        // User wants to see menu (has backgrounded sessions)
                        self.state = TerminalState::InMenu;
                    }
                }
                
                TerminalState::InMenu => {
                    // Show menu ONCE and get user choice
                    let mut menu = MainMenu::new(self.manager.clone());
                    match menu.run().await? {
                        Some(session_id) => {
                            // User selected a session
                            let _ = self.manager.set_active_session(&session_id).await;
                            
                            // Wait for session to be active, then enter shell
                            if let Some(session) = self.wait_for_active_session().await? {
                                self.state = TerminalState::InShell;
                                
                                match self.enter_interactive_mode(session).await? {
                                    InteractionResult::Detached => {
                                        // Back to menu
                                        self.state = TerminalState::InMenu;
                                    }
                                    InteractionResult::SessionEnded => {
                                        // Back to listening
                                        self.show_welcome_banner()?;
                                        self.state = TerminalState::Listening;
                                    }
                                }
                            } else {
                                // Session disappeared - back to listening
                                self.show_welcome_banner()?;
                                self.state = TerminalState::Listening;
                            }
                        }
                        None => {
                            // User quit menu - back to listening
                            self.show_welcome_banner()?;
                            self.state = TerminalState::Listening;
                        }
                    }
                }
                
                TerminalState::InShell => {
                    // This should never happen - we handle shell in other states
                    self.state = TerminalState::Listening;
                }
            }
        }
    }

    async fn wait_for_session(&self) -> Result<Option<Arc<super::session::ShellSession>>> {
        // Wait for either:
        // 1. An active session to appear
        // 2. User to have backgrounded sessions (return None to show menu)
        
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            
            // Check for active session
            if let Some(sess) = self.manager.get_active_session().await {
                // Wait for stabilization
                while *sess.is_stabilizing.read().await {
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                }
                return Ok(Some(sess));
            }
            
            // Check if user has backgrounded sessions
            if self.manager.session_count() > 0 {
                // Return None to indicate "show menu"
                return Ok(None);
            }
        }
    }

    async fn wait_for_active_session(&self) -> Result<Option<Arc<super::session::ShellSession>>> {
        // Wait for the session that was just set active to actually be active
        for _ in 0..50 {  // Timeout after 5 seconds
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            
            if let Some(sess) = self.manager.get_active_session().await {
                // Wait for stabilization
                while *sess.is_stabilizing.read().await {
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                }
                return Ok(Some(sess));
            }
        }
        
        Ok(None)
    }

    fn show_welcome_banner(&self) -> Result<()> {
        // Clear screen before showing banner
        execute!(io::stdout(), Clear(ClearType::All), cursor::MoveTo(0, 0))?;
        
        let width = get_safe_width();
        let separator = horizontal_line(width.min(80), '━');
        
        println!("{}", separator.truecolor(37, 150, 190));
        self.print_colored("◉  ", PRIMARY_COLOR);
        println!("{}", "Listening for incoming connections...".bright_white());
        println!("{}", "   Press Ctrl+C to stop listener".truecolor(120, 120, 130));
        println!("{}", separator.truecolor(37, 150, 190));
        println!();
        
        io::stdout().flush()?;
        Ok(())
    }

    fn print_colored(&self, text: &str, color: Color) {
        let _ = execute!(io::stdout(), SetForegroundColor(color), Print(text), ResetColor);
    }

    async fn enter_interactive_mode(&self, session: Arc<super::session::ShellSession>) -> Result<InteractionResult> {
        // CRITICAL: Reset terminal state completely before interactive mode
        self.reset_terminal_state()?;
        
        // Enable raw mode for transparent input
        enable_raw_mode()?;
        
        // Get terminal size and sync with remote shell
        if let Ok((cols, rows)) = size() {
            // Don't display this command - it's already stabilized
            let _ = session.send_command(format!("stty rows {} cols {} 2>/dev/null\n", rows, cols)).await;
        }
        
        let mut detach_requested = false;
        
        // Main interactive loop - pure passthrough
        loop {
            tokio::select! {
                // Handle keyboard input -> shell
                _ = tokio::time::sleep(Duration::from_millis(10)) => {
                    if event::poll(Duration::from_millis(0))? {
                        match event::read()? {
                            Event::Key(key) => {
                                if self.handle_key(key, &session).await? {
                                    detach_requested = true;
                                    break; // Detach requested
                                }
                            }
                            Event::Resize(cols, rows) => {
                                // Handle terminal resize
                                let _ = session.send_command(
                                    format!("stty rows {} cols {} 2>/dev/null\r", rows, cols)
                                ).await;
                            }
                            _ => {}
                        }
                    }
                }
                
                // Handle shell output -> stdout (direct passthrough)
                _ = tokio::time::sleep(Duration::from_millis(10)) => {
                    let mut output_rx = session.output_rx.write().await;
                    while let Ok(output) = output_rx.try_recv() {
                        print!("{}", output);
                        io::stdout().flush()?;
                    }
                }
            }
            
            // Check if session ended
            let state = session.get_state().await;
            if state == super::session::ShellState::Terminated {
                break;
            }
        }
        
        // Restore terminal
        disable_raw_mode()?;
        
        // Clear screen before showing status message
        execute!(io::stdout(), Clear(ClearType::All), cursor::MoveTo(0, 0))?;
        
        if detach_requested {
            // Background the session
            let _ = self.manager.background_session(&session.id).await;
            
            println!();
            self.print_colored("◦  ", MUTED_COLOR);
            println!("{}", "Session backgrounded - entering menu".truecolor(120, 120, 130));
            println!();
            
            io::stdout().flush()?;
            
            // Brief pause so user sees the message
            tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;
            
            Ok(InteractionResult::Detached)
        } else {
            println!();
            self.print_colored("◦  ", MUTED_COLOR);
            println!("{}", "Session ended".truecolor(120, 120, 130));
            println!();
            
            io::stdout().flush()?;
            
            // Brief pause so user sees the message
            tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;
            
            Ok(InteractionResult::SessionEnded)
        }
    }

    async fn handle_key(&self, key: KeyEvent, session: &Arc<super::session::ShellSession>) -> Result<bool> {
        match (key.code, key.modifiers) {
            // Esc or Ctrl+D = detach (return to menu)
            (KeyCode::Esc, _) | (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
                return Ok(true); // Signal detach
            }
            
            // Ctrl+C = pass to shell
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                session.send_command("\x03".to_string()).await?;
            }
            
            // Ctrl+Z = pass to shell
            (KeyCode::Char('z'), KeyModifiers::CONTROL) => {
                session.send_command("\x1a".to_string()).await?;
            }
            
            // Ctrl+L = pass to shell (clear screen)
            (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
                session.send_command("\x0c".to_string()).await?;
            }
            
            // Enter
            (KeyCode::Enter, _) => {
                session.send_command("\r\n".to_string()).await?;
            }
            
            // Tab
            (KeyCode::Tab, _) => {
                session.send_command("\t".to_string()).await?;
            }
            
            // Backspace
            (KeyCode::Backspace, _) => {
                session.send_command("\x7f".to_string()).await?;
            }
            
            // Arrow keys
            (KeyCode::Up, _) => {
                session.send_command("\x1b[A".to_string()).await?;
            }
            (KeyCode::Down, _) => {
                session.send_command("\x1b[B".to_string()).await?;
            }
            (KeyCode::Right, _) => {
                session.send_command("\x1b[C".to_string()).await?;
            }
            (KeyCode::Left, _) => {
                session.send_command("\x1b[D".to_string()).await?;
            }
            
            // Home/End
            (KeyCode::Home, _) => {
                session.send_command("\x1b[H".to_string()).await?;
            }
            (KeyCode::End, _) => {
                session.send_command("\x1b[F".to_string()).await?;
            }
            
            // Page Up/Down
            (KeyCode::PageUp, _) => {
                session.send_command("\x1b[5~".to_string()).await?;
            }
            (KeyCode::PageDown, _) => {
                session.send_command("\x1b[6~".to_string()).await?;
            }
            
            // Delete
            (KeyCode::Delete, _) => {
                session.send_command("\x1b[3~".to_string()).await?;
            }
            
            // Regular characters
            (KeyCode::Char(c), KeyModifiers::NONE) | (KeyCode::Char(c), KeyModifiers::SHIFT) => {
                session.send_command(c.to_string()).await?;
            }
            
            // Other Ctrl+ combinations
            (KeyCode::Char(c), KeyModifiers::CONTROL) => {
                let ctrl_char = ((c as u8) & 0x1f) as char;
                session.send_command(ctrl_char.to_string()).await?;
            }
            
            _ => {}
        }
        
        Ok(false) // Continue
    }

    fn reset_terminal_state(&self) -> Result<()> {
        // Disable raw mode if enabled
        let _ = disable_raw_mode();
        
        // Reset colors
        execute!(io::stdout(), ResetColor)?;
        
        // Show cursor (in case it was hidden)
        execute!(io::stdout(), cursor::Show)?;
        
        // Flush
        io::stdout().flush()?;
        
        Ok(())
    }
}
