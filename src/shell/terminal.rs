use anyhow::Result;
use colored::*;
use crossterm::{
    cursor, execute,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{disable_raw_mode, enable_raw_mode, size},
};
use std::io::{self, Write};
use std::sync::Arc;
use std::time::Duration;

use super::session::ShellSessionManager;

// Modern color scheme
const PRIMARY_COLOR: Color = Color::Rgb { r: 37, g: 150, b: 190 };    // #2596be
const SECONDARY_COLOR: Color = Color::Rgb { r: 86, g: 33, b: 213 };   // #5621d5
const SUCCESS_COLOR: Color = Color::Rgb { r: 80, g: 200, b: 120 };
const WARNING_COLOR: Color = Color::Rgb { r: 255, g: 180, b: 80 };
const MUTED_COLOR: Color = Color::Rgb { r: 120, g: 120, b: 130 };

pub struct InteractiveTerminal {
    manager: Arc<ShellSessionManager>,
}

impl InteractiveTerminal {
    pub fn new(manager: Arc<ShellSessionManager>) -> Self {
        Self { manager }
    }

    pub async fn run(&mut self) -> Result<()> {
        // Show welcome banner (no raw mode yet)
        self.show_welcome_banner()?;
        
        // Wait for a session to become active
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            
            if let Some(session) = self.manager.get_active_session().await {
                // Wait for stabilization to complete
                while *session.is_stabilizing.read().await {
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                }
                
                // Stabilization complete - now enter pure interactive mode
                self.enter_interactive_mode(session).await?;
                break;
            }
        }
        
        Ok(())
    }

    fn show_welcome_banner(&self) -> Result<()> {
        println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".truecolor(37, 150, 190));
        self.print_colored("◉  ", PRIMARY_COLOR);
        println!("{}", "Listening for incoming connections...".bright_white());
        println!("{}", "   Press Ctrl+C to stop listener".truecolor(120, 120, 130));
        println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".truecolor(37, 150, 190));
        println!();
        Ok(())
    }

    fn print_colored(&self, text: &str, color: Color) {
        let _ = execute!(io::stdout(), SetForegroundColor(color), Print(text), ResetColor);
    }

    async fn enter_interactive_mode(&self, session: Arc<super::session::ShellSession>) -> Result<()> {
        // CRITICAL: Reset terminal state completely before interactive mode
        self.reset_terminal_state()?;
        
        // Enable raw mode for transparent input
        enable_raw_mode()?;
        
        // Get terminal size and sync with remote shell
        if let Ok((cols, rows)) = size() {
            // Don't display this command - it's already stabilized
            let _ = session.send_command(format!("stty rows {} cols {} 2>/dev/null\n", rows, cols)).await;
        }
        
        // Main interactive loop - pure passthrough
        loop {
            tokio::select! {
                // Handle keyboard input -> shell
                _ = tokio::time::sleep(Duration::from_millis(10)) => {
                    if event::poll(Duration::from_millis(0))? {
                        match event::read()? {
                            Event::Key(key) => {
                                if self.handle_key(key, &session).await? {
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
        self.reset_terminal_state()?;
        
        println!();
        self.print_colored("◦  ", MUTED_COLOR);
        println!("{}", "Session ended".truecolor(120, 120, 130));
        println!();
        
        Ok(())
    }

    async fn handle_key(&self, key: KeyEvent, session: &Arc<super::session::ShellSession>) -> Result<bool> {
        match (key.code, key.modifiers) {
            // Ctrl+D = detach (special CAP command)
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
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
