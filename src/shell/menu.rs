use anyhow::Result;
use colored::*;
use crossterm::{
    cursor, execute,
    event::{self, Event, KeyCode},
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
};
use std::io::{self, Write};
use std::sync::Arc;
use std::time::Duration;

use super::session::{ShellSessionManager, ShellState};

const PRIMARY_COLOR: Color = Color::Rgb { r: 37, g: 150, b: 190 };
const SECONDARY_COLOR: Color = Color::Rgb { r: 86, g: 33, b: 213 };
const SUCCESS_COLOR: Color = Color::Rgb { r: 80, g: 200, b: 120 };
const WARNING_COLOR: Color = Color::Rgb { r: 255, g: 180, b: 80 };
const MUTED_COLOR: Color = Color::Rgb { r: 120, g: 120, b: 130 };

pub struct MainMenu {
    manager: Arc<ShellSessionManager>,
    selected: usize,
}

impl MainMenu {
    pub fn new(manager: Arc<ShellSessionManager>) -> Self {
        Self {
            manager,
            selected: 0,
        }
    }

    fn print_colored(&self, text: &str, color: Color) {
        let _ = execute!(io::stdout(), SetForegroundColor(color), Print(text), ResetColor);
    }

    pub async fn run(&mut self) -> Result<Option<String>> {
        // Enable raw mode for menu
        enable_raw_mode()?;
        
        let result = self.run_menu_loop().await;
        
        // Disable raw mode when leaving
        disable_raw_mode()?;
        
        result
    }

    async fn run_menu_loop(&mut self) -> Result<Option<String>> {
        let mut sessions = self.get_session_list().await;
        
        if sessions.is_empty() {
            // No sessions - show message and wait
            self.render_empty_screen()?;
            
            // Wait for key to exit
            loop {
                if event::poll(Duration::from_millis(100))? {
                    if let Event::Key(key) = event::read()? {
                        if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
                            return Ok(None);
                        }
                    }
                }
            }
        }
        
        // Initial render
        self.render_menu(&sessions)?;
        
        // Event loop - only redraw on actual user input
        loop {
            if let Some(event) = self.wait_for_event().await? {
                match event {
                    MenuEvent::SelectNext => {
                        if self.selected < sessions.len() - 1 {
                            self.selected += 1;
                            self.render_menu(&sessions)?;
                        }
                    }
                    MenuEvent::SelectPrev => {
                        if self.selected > 0 {
                            self.selected -= 1;
                            self.render_menu(&sessions)?;
                        }
                    }
                    MenuEvent::Confirm => {
                        return Ok(Some(sessions[self.selected].id.clone()));
                    }
                    MenuEvent::Kill => {
                        // Kill selected session
                        let session_id = &sessions[self.selected].id;
                        let _ = self.manager.terminate_session(session_id).await;
                        
                        // Refresh session list
                        sessions = self.get_session_list().await;
                        if sessions.is_empty() {
                            self.render_empty_screen()?;
                            // Continue to wait for quit
                            loop {
                                if event::poll(Duration::from_millis(100))? {
                                    if let Event::Key(key) = event::read()? {
                                        if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
                                            return Ok(None);
                                        }
                                    }
                                }
                            }
                        }
                        
                        // Adjust selection if needed
                        self.selected = self.selected.min(sessions.len().saturating_sub(1));
                        self.render_menu(&sessions)?;
                    }
                    MenuEvent::Quit => {
                        return Ok(None);
                    }
                }
            }
        }
    }

    async fn wait_for_event(&self) -> Result<Option<MenuEvent>> {
        // Block until we get a key event
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                return Ok(match key.code {
                    KeyCode::Up | KeyCode::Char('k') => Some(MenuEvent::SelectPrev),
                    KeyCode::Down | KeyCode::Char('j') => Some(MenuEvent::SelectNext),
                    KeyCode::Enter => Some(MenuEvent::Confirm),
                    KeyCode::Char('K') => Some(MenuEvent::Kill),
                    KeyCode::Char('q') | KeyCode::Esc => Some(MenuEvent::Quit),
                    _ => None,
                });
            }
        }
        Ok(None)
    }

    fn render_menu(&self, sessions: &[SessionInfo]) -> Result<()> {
        // CRITICAL: Clear entire screen before rendering
        execute!(io::stdout(), Clear(ClearType::All), cursor::MoveTo(0, 0))?;
        
        // Render header
        self.render_header()?;
        
        // Render session list
        println!();
        println!("{}", "  Active Sessions:".bright_white());
        println!();
        
        for (idx, session) in sessions.iter().enumerate() {
            self.render_session_line(session, idx == self.selected)?;
        }
        
        // Render footer
        println!();
        self.render_footer()?;
        
        io::stdout().flush()?;
        Ok(())
    }

    fn render_empty_screen(&self) -> Result<()> {
        // CRITICAL: Clear entire screen before rendering
        execute!(io::stdout(), Clear(ClearType::All), cursor::MoveTo(0, 0))?;
        
        self.render_header()?;
        
        println!();
        self.print_colored("  ◦ ", MUTED_COLOR);
        println!("{}", "No active sessions".truecolor(120, 120, 130));
        println!();
        println!("{}", "  Press 'q' to exit or wait for incoming connections".truecolor(120, 120, 130));
        
        io::stdout().flush()?;
        Ok(())
    }

    fn render_header(&self) -> Result<()> {
        println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".truecolor(37, 150, 190));
        self.print_colored("  CAP ", PRIMARY_COLOR);
        println!("{}", "Session Manager".bright_white());
        println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".truecolor(37, 150, 190));
        Ok(())
    }

    fn render_footer(&self) -> Result<()> {
        println!("{}", "  Commands:".truecolor(120, 120, 130));
        self.print_colored("    ↑↓", SECONDARY_COLOR);
        print!(" Select  ");
        self.print_colored("Enter", SECONDARY_COLOR);
        print!(" Interact  ");
        self.print_colored("K", SECONDARY_COLOR);
        print!(" Kill  ");
        self.print_colored("q/Esc", SECONDARY_COLOR);
        print!(" Back");
        println!();
        Ok(())
    }

    fn render_session_line(&self, session: &SessionInfo, is_selected: bool) -> Result<()> {
        // Selection indicator
        if is_selected {
            self.print_colored("  ▸ ", PRIMARY_COLOR);
        } else {
            print!("    ");
        }
        
        // Session ID
        if is_selected {
            self.print_colored(&format!("<{}>", session.short_id), SECONDARY_COLOR);
        } else {
            print!("{}", format!("<{}>", session.short_id).truecolor(100, 100, 110));
        }
        
        // Status
        let status_str = match session.state {
            ShellState::Active => {
                if is_selected {
                    format!(" [{}]", "Active".bright_green())
                } else {
                    format!(" [{}]", "Active".truecolor(80, 200, 120))
                }
            }
            ShellState::Background => {
                format!(" [{}]", "Background".truecolor(255, 180, 80))
            }
            ShellState::Terminated => {
                format!(" [{}]", "Terminated".truecolor(200, 80, 80))
            }
        };
        print!("{}", status_str);
        
        // Remote address
        print!(" from {}", session.remote_addr.bright_white());
        
        // Metadata
        if let Some(ref user) = session.username {
            print!(" - {}", user.bright_cyan());
            if let Some(ref host) = session.hostname {
                print!("@{}", host.bright_cyan());
            }
        }
        
        if let Some(ref os) = session.os_type {
            print!(" ({})", os.truecolor(120, 120, 130));
        }
        
        println!();
        Ok(())
    }

    async fn get_session_list(&self) -> Vec<SessionInfo> {
        let mut sessions = Vec::new();
        
        for entry in self.manager.list_sessions() {
            if let Some(session) = self.manager.get_session(&entry.0) {
                let metadata = session.get_metadata().await;
                let state = session.get_state().await;
                
                sessions.push(SessionInfo {
                    id: entry.0.clone(),
                    short_id: entry.0[..8].to_string(),
                    remote_addr: metadata.remote_addr.clone(),
                    hostname: metadata.hostname.clone(),
                    username: metadata.username.clone(),
                    os_type: metadata.os_type.clone(),
                    state,
                });
            }
        }
        
        sessions
    }
}

enum MenuEvent {
    SelectNext,
    SelectPrev,
    Confirm,
    Kill,
    Quit,
}

#[derive(Clone)]
struct SessionInfo {
    id: String,
    short_id: String,
    remote_addr: String,
    hostname: Option<String>,
    username: Option<String>,
    os_type: Option<String>,
    state: ShellState,
}
