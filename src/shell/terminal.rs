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

    /// Run interactive session - TRUE PASSTHROUGH MODE
    /// Becomes a transparent bridge: stdin → remote shell → stdout
    async fn run_session(&mut self, session: Arc<super::session::ShellSession>) -> Result<InteractionResult> {
        // Set terminal size on remote
        if let Ok((cols, rows)) = size() {
            let _ = session.send_command(format!("stty rows {} cols {} 2>/dev/null\n", rows, cols)).await;
        }
        
        // Signal handler for terminal resize
        let resize_session = session.clone();
        tokio::spawn(async move {
            use signal_hook::consts::SIGWINCH;
            use signal_hook_tokio::Signals;
            use futures::stream::StreamExt;
            
            if let Ok(mut signals) = Signals::new(&[SIGWINCH]) {
                while signals.next().await.is_some() {
                    if let Ok((cols, rows)) = size() {
                        let _ = resize_session.send_command(
                            format!("stty rows {} cols {} 2>/dev/null\r", rows, cols)
                        ).await;
                    }
                }
            }
        });
        
        let mut detach_requested = false;
        let mut esc_sequence = Vec::new();
        let esc_timeout = Duration::from_millis(100);
        let mut last_byte_time = std::time::Instant::now();
        
        // TRUE PASSTHROUGH LOOP - byte-for-byte forwarding
        loop {
            tokio::select! {
                // Keyboard input → remote shell (byte-for-byte, no interpretation)
                _ = tokio::time::sleep(Duration::from_millis(1)) => {
                    // Poll for keyboard events in raw mode
                    if event::poll(Duration::from_millis(0))? {
                        match event::read()? {
                            Event::Key(key) => {
                                // Convert key to raw bytes
                                let bytes = self.key_to_bytes(key);
                                
                                // Special case: bare Esc key for detachment
                                if bytes == vec![0x1b] {
                                    // Start Esc sequence detection
                                    esc_sequence.clear();
                                    esc_sequence.push(0x1b);
                                    last_byte_time = std::time::Instant::now();
                                    continue;
                                }
                                
                                // If we have Esc sequence in progress, check if it's expanding
                                if !esc_sequence.is_empty() {
                                    if bytes.starts_with(&[0x1b]) && bytes.len() > 1 {
                                        // This is an escape sequence (arrow key, etc.), not bare Esc
                                        esc_sequence.clear();
                                        // Fall through to send it
                                    } else if last_byte_time.elapsed() > esc_timeout {
                                        // Bare Esc timed out - detach
                                        detach_requested = true;
                                        break;
                                    }
                                }
                                
                                // Forward bytes EXACTLY as received - direct TCP write
                                if let Err(_) = session.write_raw_bytes(&bytes).await {
                                    break; // Session died
                                }
                                
                                // Clear Esc sequence since we sent something else
                                esc_sequence.clear();
                            }
                            Event::Resize(cols, rows) => {
                                let _ = session.send_command(
                                    format!("stty rows {} cols {} 2>/dev/null\r", rows, cols)
                                ).await;
                            }
                            _ => {}
                        }
                    } else if !esc_sequence.is_empty() && last_byte_time.elapsed() > esc_timeout {
                        // Bare Esc timed out - detach
                        detach_requested = true;
                        break;
                    }
                }
                
                // Remote shell output → stdout (direct, no buffering, no formatting)
                _ = tokio::time::sleep(Duration::from_millis(1)) => {
                    if self.mode == TerminalMode::SessionActive {
                        let mut output_rx = session.output_rx.write().await;
                        
                        // Drain ALL available output immediately
                        let mut has_output = false;
                        while let Ok(output) = output_rx.try_recv() {
                            // Write directly to stdout - zero interpretation
                            print!("{}", output);
                            has_output = true;
                        }
                        
                        // Flush immediately if we got any output
                        if has_output {
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
    
    /// Convert KeyEvent to raw bytes for true passthrough
    fn key_to_bytes(&self, key: KeyEvent) -> Vec<u8> {
        match (key.code, key.modifiers) {
            // Ctrl+C
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => vec![0x03],
            // Ctrl+Z
            (KeyCode::Char('z'), KeyModifiers::CONTROL) => vec![0x1a],
            // Ctrl+D
            (KeyCode::Char('d'), KeyModifiers::CONTROL) => vec![0x04],
            // Ctrl+L
            (KeyCode::Char('l'), KeyModifiers::CONTROL) => vec![0x0c],
            // Other Ctrl combinations
            (KeyCode::Char(c), KeyModifiers::CONTROL) => {
                vec![((c as u8) & 0x1f)]
            }
            // Enter → \r (not \n, let remote handle it)
            (KeyCode::Enter, _) => vec![0x0d],
            // Tab
            (KeyCode::Tab, _) => vec![0x09],
            // Backspace
            (KeyCode::Backspace, _) => vec![0x7f],
            // Bare Esc
            (KeyCode::Esc, _) => vec![0x1b],
            // Arrow keys (ANSI sequences)
            (KeyCode::Up, _) => vec![0x1b, 0x5b, 0x41],
            (KeyCode::Down, _) => vec![0x1b, 0x5b, 0x42],
            (KeyCode::Right, _) => vec![0x1b, 0x5b, 0x43],
            (KeyCode::Left, _) => vec![0x1b, 0x5b, 0x44],
            // Home/End
            (KeyCode::Home, _) => vec![0x1b, 0x5b, 0x48],
            (KeyCode::End, _) => vec![0x1b, 0x5b, 0x46],
            // Page Up/Down
            (KeyCode::PageUp, _) => vec![0x1b, 0x5b, 0x35, 0x7e],
            (KeyCode::PageDown, _) => vec![0x1b, 0x5b, 0x36, 0x7e],
            // Delete
            (KeyCode::Delete, _) => vec![0x1b, 0x5b, 0x33, 0x7e],
            // Insert
            (KeyCode::Insert, _) => vec![0x1b, 0x5b, 0x32, 0x7e],
            // Regular characters (with shift handled by crossterm)
            (KeyCode::Char(c), _) => c.to_string().as_bytes().to_vec(),
            // Everything else - ignore
            _ => vec![],
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
