use anyhow::Result;
use colored::Colorize;
use crossterm::{
    execute,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::size,
};
use nix::sys::termios::{self, LocalFlags, InputFlags, OutputFlags, ControlFlags, SetArg};
use nix::poll::{poll, PollFd, PollFlags};
use std::io::{self, Write, Read};
use std::os::unix::io::{AsRawFd, RawFd};
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
        
        // STEP 1: Save current terminal settings and enter raw mode
        use std::os::fd::AsFd;
        let stdin = io::stdin();
        let original_termios = termios::tcgetattr(&stdin)?;
        
        // Configure raw mode manually (like cfmakeraw)
        let mut raw_termios = original_termios.clone();
        
        // Input flags: no break, no CR to NL, no parity check, no strip char, no start/stop output control
        raw_termios.input_flags &= !(InputFlags::BRKINT | InputFlags::ICRNL | InputFlags::INPCK | 
                                       InputFlags::ISTRIP | InputFlags::IXON);
        
        // Output flags: disable post processing
        raw_termios.output_flags &= !OutputFlags::OPOST;
        
        // Control flags: set 8 bit chars
        raw_termios.control_flags |= ControlFlags::CS8;
        
        // Local flags: no echo, no canonical mode, no extended functions, no signal chars
        raw_termios.local_flags &= !(LocalFlags::ECHO | LocalFlags::ICANON | LocalFlags::IEXTEN | 
                                      LocalFlags::ISIG);
        
        // Set raw mode NOW
        termios::tcsetattr(&stdin, SetArg::TCSANOW, &raw_termios)?;
        
        // STEP 2: Direct file descriptor I/O loop
        let result = self.run_passthrough_loop(session.clone()).await;
        
        // STEP 3: Always restore terminal settings
        termios::tcsetattr(&stdin, SetArg::TCSANOW, &original_termios)?;
        
        result
    }
    
    /// TRUE PASSTHROUGH LOOP using raw file descriptors
    /// This is how professional tools like Penelope, tmux, and ssh work
    async fn run_passthrough_loop(
        &mut self, 
        session: Arc<super::session::ShellSession>,
    ) -> Result<InteractionResult> {
        use std::os::fd::AsFd;
        
        let mut stdin_buffer = [0u8; 4096];
        let mut esc_buffer = Vec::new();
        let mut esc_timer = std::time::Instant::now();
        let esc_timeout = Duration::from_millis(100);
        
        let stdin = io::stdin();
        
        loop {
            // STEP 1: Check for stdin input (non-blocking)
            let mut poll_fds = [PollFd::new(&stdin, PollFlags::POLLIN)];
            
            match poll(&mut poll_fds, 1) {  // 1ms timeout
                Ok(n) if n > 0 && poll_fds[0].revents().is_some() => {
                    // stdin has data - read it
                    let mut stdin_lock = stdin.lock();
                    match stdin_lock.read(&mut stdin_buffer) {
                        Ok(0) => break,  // EOF
                        Ok(n) => {
                            let bytes = &stdin_buffer[..n];
                            
                            // Esc detection for detachment
                            if n == 1 && bytes[0] == 0x1b {
                                if esc_buffer.is_empty() {
                                    // First Esc byte
                                    esc_buffer.push(0x1b);
                                    esc_timer = std::time::Instant::now();
                                    continue;
                                }
                            }
                            
                            // If we had an Esc waiting and got more bytes, it's an escape sequence
                            if !esc_buffer.is_empty() {
                                if bytes[0] != 0x1b {
                                    // Not bare Esc - it's an escape sequence, forward it
                                    esc_buffer.extend_from_slice(bytes);
                                    session.write_raw_bytes(&esc_buffer).await?;
                                    esc_buffer.clear();
                                    continue;
                                }
                            }
                            
                            // Normal input - forward directly
                            session.write_raw_bytes(bytes).await?;
                            esc_buffer.clear();
                        }
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                            // No data available, continue
                        }
                        Err(_) => break,
                    }
                }
                _ => {}
            }
            
            // Check if bare Esc timed out → detach
            if !esc_buffer.is_empty() && esc_timer.elapsed() > esc_timeout {
                let _ = self.manager.background_session(&session.id).await;
                return Ok(InteractionResult::Detached);
            }
            
            // STEP 2: Check for session output
            if self.mode == TerminalMode::SessionActive {
                let mut output_rx = session.output_rx.write().await;
                
                // Drain all available output
                while let Ok(output) = output_rx.try_recv() {
                    // Write directly to stdout (fd 1)
                    let mut stdout = io::stdout();
                    stdout.write_all(output.as_bytes())?;
                    stdout.flush()?;
                }
            }
            
            // STEP 3: Check if session terminated
            let state = session.get_state().await;
            if state == super::session::ShellState::Terminated {
                return Ok(InteractionResult::SessionEnded);
            }
            
            // Small sleep to prevent tight looping
            tokio::time::sleep(Duration::from_micros(100)).await;
        }
        
        Ok(InteractionResult::SessionEnded)
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
