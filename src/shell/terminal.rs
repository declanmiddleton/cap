use anyhow::Result;
use colored::Colorize;
use crossterm::{
    execute,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::size,
};
use std::io::{self, Write};
use std::sync::Arc;
use std::time::Duration;

use super::session::ShellSessionManager;
use super::menu::MainMenu;
use super::formatting::{get_safe_width, horizontal_line};
use super::renderer::{TerminalRenderer, OutputBuffer};

const PRIMARY_COLOR: Color = Color::Rgb { r: 37, g: 150, b: 190 };
const MUTED_COLOR: Color = Color::Rgb { r: 120, g: 120, b: 130 };

pub enum InteractionResult {
    Detached,      // User pressed Esc to return to menu
    SessionEnded,  // Session terminated
}

/// Terminal state - ONLY ONE can be active at a time
#[derive(Debug, PartialEq, Eq)]
enum TerminalMode {
    Listening,      // Waiting for connections (main screen, no raw mode)
    SessionActive,  // Interactive shell (main screen, raw mode, session owns output)
    MenuActive,     // Menu displayed (alternate screen, raw mode, menu owns output)
}

pub struct InteractiveTerminal {
    manager: Arc<ShellSessionManager>,
    renderer: TerminalRenderer,
    mode: TerminalMode,
    output_buffer: OutputBuffer,
}

impl InteractiveTerminal {
    pub fn new(manager: Arc<ShellSessionManager>) -> Self {
        Self { 
            manager,
            renderer: TerminalRenderer::new(),
            mode: TerminalMode::Listening,
            output_buffer: OutputBuffer::new(10000), // Buffer up to 10k lines
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        // Start in listening mode
        self.transition_to_listening()?;
        self.show_welcome_banner()?;
        
        loop {
            match self.mode {
                TerminalMode::Listening => {
                    // Wait for session or user requesting menu
                    if let Some(session) = self.wait_for_session().await? {
                        // Transition to session mode
                        self.transition_to_session_active()?;
                        
                        match self.run_session(session).await? {
                            InteractionResult::Detached => {
                                // User wants menu - transition to alternate screen
                                self.transition_to_menu_active()?;
                                self.run_menu().await?;
                            }
                            InteractionResult::SessionEnded => {
                                // Session ended - back to listening
                                self.transition_to_listening()?;
                                self.show_welcome_banner()?;
                            }
                        }
                    } else {
                        // User has backgrounded sessions - show menu
                        self.transition_to_menu_active()?;
                        self.run_menu().await?;
                    }
                }
                
                TerminalMode::MenuActive => {
                    // Should not get here - run_menu handles this
                    self.transition_to_listening()?;
                }
                
                TerminalMode::SessionActive => {
                    // Should not get here - run_session handles this
                    self.transition_to_listening()?;
                }
            }
        }
    }

    /// STRICT: Transition to listening mode
    fn transition_to_listening(&mut self) -> Result<()> {
        self.renderer.transition_to_listening()?;
        self.mode = TerminalMode::Listening;
        Ok(())
    }

    /// STRICT: Transition to session active mode
    fn transition_to_session_active(&mut self) -> Result<()> {
        self.renderer.transition_to_session()?;
        self.mode = TerminalMode::SessionActive;
        
        // Flush any buffered output from previous menu session
        if self.output_buffer.has_content() {
            self.output_buffer.flush_to_stdout()?;
        }
        
        Ok(())
    }

    /// STRICT: Transition to menu active mode  
    fn transition_to_menu_active(&mut self) -> Result<()> {
        self.renderer.transition_to_menu()?;
        self.mode = TerminalMode::MenuActive;
        
        // Clear output buffer - we're in alternate screen now
        self.output_buffer.clear();
        
        Ok(())
    }

    async fn wait_for_session(&self) -> Result<Option<Arc<super::session::ShellSession>>> {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            
            // Check for active session
            if let Some(sess) = self.manager.get_active_session().await {
                while *sess.is_stabilizing.read().await {
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                }
                return Ok(Some(sess));
            }
            
            // Check if user has backgrounded sessions
            if self.manager.session_count() > 0 {
                return Ok(None);
            }
        }
    }

    fn show_welcome_banner(&self) -> Result<()> {
        self.renderer.clear_screen()?;
        
        let width = get_safe_width();
        let separator = horizontal_line(width.min(80), '━');
        
        println!("{}", separator.truecolor(37, 150, 190));
        self.print_colored("◉  ", PRIMARY_COLOR);
        println!("{}", "Listening for incoming connections...".bright_white());
        println!("{}", "   Press Ctrl+C to stop listener".truecolor(120, 120, 130));
        println!("{}", separator.truecolor(37, 150, 190));
        println!();
        
        self.renderer.flush()?;
        Ok(())
    }

    /// Run interactive session - EXCLUSIVE terminal ownership
    async fn run_session(&mut self, session: Arc<super::session::ShellSession>) -> Result<InteractionResult> {
        // Session owns terminal completely
        // Set terminal size
        if let Ok((cols, rows)) = size() {
            let _ = session.send_command(format!("stty rows {} cols {} 2>/dev/null\n", rows, cols)).await;
        }
        
        let mut detach_requested = false;
        
        // Pure passthrough loop
        loop {
            tokio::select! {
                // Handle keyboard -> shell
                _ = tokio::time::sleep(Duration::from_millis(10)) => {
                    if event::poll(Duration::from_millis(0))? {
                        match event::read()? {
                            Event::Key(key) => {
                                if self.handle_key(key, &session).await? {
                                    detach_requested = true;
                                    break;
                                }
                            }
                            Event::Resize(cols, rows) => {
                                let _ = session.send_command(
                                    format!("stty rows {} cols {} 2>/dev/null\r", rows, cols)
                                ).await;
                            }
                            _ => {}
                        }
                    }
                }
                
                // Handle shell output -> stdout (ONLY in SessionActive mode)
                _ = tokio::time::sleep(Duration::from_millis(10)) => {
                    if self.mode == TerminalMode::SessionActive {
                        let mut output_rx = session.output_rx.write().await;
                        while let Ok(output) = output_rx.try_recv() {
                            // Direct write - we own the terminal
                            print!("{}", output);
                            io::stdout().flush()?;
                        }
                    }
                }
            }
            
            // Check if session ended
            let state = session.get_state().await;
            if state == super::session::ShellState::Terminated {
                break;
            }
        }
        
        if detach_requested {
            let _ = self.manager.background_session(&session.id).await;
            Ok(InteractionResult::Detached)
        } else {
            Ok(InteractionResult::SessionEnded)
        }
    }

    /// Run menu - EXCLUSIVE terminal ownership (alternate screen)
    async fn run_menu(&mut self) -> Result<()> {
        // Menu owns alternate screen completely - loop to handle session cycling
        loop {
            let mut menu = MainMenu::new(self.manager.clone());
            
            match menu.run().await? {
                Some(session_id) => {
                    // User selected session
                    let _ = self.manager.set_active_session(&session_id).await;
                    
                    // Wait for session to be active
                    let mut session_found = false;
                    for _ in 0..50 {
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        if let Some(sess) = self.manager.get_active_session().await {
                            while *sess.is_stabilizing.read().await {
                                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                            }
                            
                            // Transition back to session
                            self.transition_to_session_active()?;
                            
                            // Run the session
                            match self.run_session(sess).await? {
                                InteractionResult::Detached => {
                                    // Back to menu - re-enter alternate screen and continue loop
                                    self.transition_to_menu_active()?;
                                    session_found = true;
                                    break;
                                }
                                InteractionResult::SessionEnded => {
                                    // Back to listening
                                    self.transition_to_listening()?;
                                    self.show_welcome_banner()?;
                                    return Ok(());
                                }
                            }
                        }
                    }
                    
                    if !session_found {
                        // Session disappeared - back to listening
                        self.transition_to_listening()?;
                        self.show_welcome_banner()?;
                        return Ok(());
                    }
                    
                    // Continue menu loop for next selection
                }
                None => {
                    // User quit menu - back to listening
                    self.transition_to_listening()?;
                    self.show_welcome_banner()?;
                    return Ok(());
                }
            }
        }
    }

    async fn handle_key(&self, key: KeyEvent, session: &Arc<super::session::ShellSession>) -> Result<bool> {
        match (key.code, key.modifiers) {
            // Esc = detach (return to menu)
            (KeyCode::Esc, _) => Ok(true),
            
            // Ctrl+C = pass to shell
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                session.send_command("\x03".to_string()).await?;
                Ok(false)
            }
            
            // Ctrl+Z = pass to shell
            (KeyCode::Char('z'), KeyModifiers::CONTROL) => {
                session.send_command("\x1a".to_string()).await?;
                Ok(false)
            }
            
            // Ctrl+L = pass to shell
            (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
                session.send_command("\x0c".to_string()).await?;
                Ok(false)
            }
            
            // Enter
            (KeyCode::Enter, _) => {
                session.send_command("\r\n".to_string()).await?;
                Ok(false)
            }
            
            // Tab
            (KeyCode::Tab, _) => {
                session.send_command("\t".to_string()).await?;
                Ok(false)
            }
            
            // Backspace
            (KeyCode::Backspace, _) => {
                session.send_command("\x7f".to_string()).await?;
                Ok(false)
            }
            
            // Arrow keys
            (KeyCode::Up, _) => {
                session.send_command("\x1b[A".to_string()).await?;
                Ok(false)
            }
            (KeyCode::Down, _) => {
                session.send_command("\x1b[B".to_string()).await?;
                Ok(false)
            }
            (KeyCode::Right, _) => {
                session.send_command("\x1b[C".to_string()).await?;
                Ok(false)
            }
            (KeyCode::Left, _) => {
                session.send_command("\x1b[D".to_string()).await?;
                Ok(false)
            }
            
            // Home/End
            (KeyCode::Home, _) => {
                session.send_command("\x1b[H".to_string()).await?;
                Ok(false)
            }
            (KeyCode::End, _) => {
                session.send_command("\x1b[F".to_string()).await?;
                Ok(false)
            }
            
            // Page Up/Down
            (KeyCode::PageUp, _) => {
                session.send_command("\x1b[5~".to_string()).await?;
                Ok(false)
            }
            (KeyCode::PageDown, _) => {
                session.send_command("\x1b[6~".to_string()).await?;
                Ok(false)
            }
            
            // Delete
            (KeyCode::Delete, _) => {
                session.send_command("\x1b[3~".to_string()).await?;
                Ok(false)
            }
            
            // Regular characters
            (KeyCode::Char(c), KeyModifiers::NONE) | (KeyCode::Char(c), KeyModifiers::SHIFT) => {
                session.send_command(c.to_string()).await?;
                Ok(false)
            }
            
            // Other Ctrl+ combinations
            (KeyCode::Char(c), KeyModifiers::CONTROL) => {
                let ctrl_char = ((c as u8) & 0x1f) as char;
                session.send_command(ctrl_char.to_string()).await?;
                Ok(false)
            }
            
            _ => Ok(false)
        }
    }

    fn print_colored(&self, text: &str, color: Color) {
        let _ = execute!(io::stdout(), SetForegroundColor(color), Print(text), ResetColor);
    }
}

impl Drop for InteractiveTerminal {
    fn drop(&mut self) {
        // Ensure cleanup
        let _ = self.renderer.cleanup();
    }
}
