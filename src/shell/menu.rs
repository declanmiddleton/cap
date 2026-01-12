use anyhow::Result;
use colored::*;
use crossterm::{
    cursor, execute,
    event::{self, Event, KeyCode, KeyEvent},
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{Clear, ClearType, size},
};
use std::io::{self, Write};
use std::sync::Arc;

use super::session::ShellSessionManager;
use super::formatting::{get_safe_width, horizontal_line};

const PRIMARY_COLOR: Color = Color::Rgb { r: 37, g: 150, b: 190 };
const SUCCESS_COLOR: Color = Color::Rgb { r: 46, g: 204, b: 113 };
const DANGER_COLOR: Color = Color::Rgb { r: 231, g: 76, b: 60 };
const MUTED_COLOR: Color = Color::Rgb { r: 120, g: 120, b: 130 };

struct SessionInfo {
    id: String,
    target: String,
    user: String,
    os: String,
    status: String,
}

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

    /// Run menu with FULL-FRAME rendering
    /// Returns Some(session_id) if user selected a session, None if quit
    pub async fn run(&mut self) -> Result<Option<String>> {
        loop {
            // FULL FRAME RENDER - clear everything and redraw
            self.render_frame().await?;
            
            // Wait for user input (event-driven, not polling)
            if let Event::Key(key) = event::read()? {
                match self.handle_key(key).await? {
                    MenuAction::SelectSession(id) => return Ok(Some(id)),
                    MenuAction::Quit => return Ok(None),
                    MenuAction::Continue => {
                        // Re-render on next loop
                    }
                }
            }
        }
    }

    /// ATOMIC FULL-FRAME RENDER - called once per user action
    async fn render_frame(&self) -> Result<()> {
        // Get terminal size
        let (term_width, _term_height) = size()?;
        let width = get_safe_width().min(term_width as usize);
        
        // Clear screen and reset cursor
        execute!(io::stdout(), Clear(ClearType::All), cursor::MoveTo(0, 0))?;
        
        // Header
        let separator = horizontal_line(width.min(80), '━');
        println!("{}", separator.truecolor(37, 150, 190));
        self.print_colored("◉  ", PRIMARY_COLOR);
        println!("{}", "CAP Session Manager".bright_white().bold());
        println!("{}", separator.truecolor(37, 150, 190));
        println!();
        
        // Get sessions
        let sessions = self.get_session_list().await;
        
        if sessions.is_empty() {
            println!("{}", "   No active sessions".truecolor(120, 120, 130));
            println!();
            println!("{}", "   Waiting for connections...".truecolor(120, 120, 130));
        } else {
            println!("{}", "   Active Sessions:".bright_white());
            println!();
            
            for (idx, sess) in sessions.iter().enumerate() {
                self.draw_session_entry(idx, sess, idx == self.selected, width)?;
            }
        }
        
        println!();
        
        // Footer with instructions
        println!("{}", separator.truecolor(37, 150, 190));
        if !sessions.is_empty() {
            println!("{}", "   ↑↓: Navigate  │  Enter: Interact  │  K: Kill  │  Q/Esc: Quit".truecolor(120, 120, 130));
        } else {
            println!("{}", "   Q/Esc: Quit".truecolor(120, 120, 130));
        }
        println!("{}", separator.truecolor(37, 150, 190));
        
        // Flush once - atomic render complete
        io::stdout().flush()?;
        
        Ok(())
    }

    async fn get_session_list(&self) -> Vec<SessionInfo> {
        let count = self.manager.session_count();
        let mut sessions = Vec::new();
        
        for i in 0..count {
            if let Some(id) = self.manager.get_session_id_by_index(i) {
                if let Some(sess) = self.manager.get_session(&id) {
                    let metadata = sess.metadata.read().await;
                    sessions.push(SessionInfo {
                        id,
                        target: metadata.remote_addr.clone(),
                        user: metadata.username.clone().unwrap_or_else(|| "unknown".to_string()),
                        os: metadata.os_type.clone().unwrap_or_else(|| "unknown".to_string()),
                        status: if *sess.is_stabilizing.read().await {
                            "stabilizing".to_string()
                        } else {
                            "active".to_string()
                        },
                    });
                }
            }
        }
        
        sessions
    }

    fn draw_session_entry(&self, _idx: usize, session: &SessionInfo, is_selected: bool, _width: usize) -> Result<()> {
        let prefix = if is_selected { " ▶ " } else { "   " };
        
        let status_color = match session.status.as_str() {
            "active" => SUCCESS_COLOR,
            "stabilizing" => PRIMARY_COLOR,
            _ => MUTED_COLOR,
        };
        
        // Session ID (first 8 chars)
        let short_id = if session.id.len() >= 8 {
            &session.id[..8]
        } else {
            &session.id
        };
        
        self.print_colored(prefix, if is_selected { PRIMARY_COLOR } else { MUTED_COLOR });
        
        print!("{}", "[".truecolor(120, 120, 130));
        self.print_colored(short_id, PRIMARY_COLOR);
        print!("{}", "]".truecolor(120, 120, 130));
        
        print!(" ");
        self.print_colored(&session.status, status_color);
        
        if !session.target.is_empty() {
            print!(" │ {}", session.target.bright_white());
        }
        
        if session.user != "unknown" {
            print!(" │ {}@{}", session.user.truecolor(46, 204, 113), session.os.truecolor(120, 120, 130));
        }
        
        println!();
        
        Ok(())
    }

    async fn handle_key(&mut self, key: KeyEvent) -> Result<MenuAction> {
        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                Ok(MenuAction::Quit)
            }
            
            KeyCode::Up | KeyCode::Char('k') => {
                let count = self.manager.session_count();
                if count > 0 && self.selected > 0 {
                    self.selected -= 1;
                }
                Ok(MenuAction::Continue)
            }
            
            KeyCode::Down | KeyCode::Char('j') => {
                let count = self.manager.session_count();
                if count > 0 && self.selected < count - 1 {
                    self.selected += 1;
                }
                Ok(MenuAction::Continue)
            }
            
            KeyCode::Enter => {
                if let Some(id) = self.manager.get_session_id_by_index(self.selected) {
                    Ok(MenuAction::SelectSession(id))
                } else {
                    Ok(MenuAction::Continue)
                }
            }
            
            KeyCode::Char('K') => {
                if let Some(id) = self.manager.get_session_id_by_index(self.selected) {
                    let _ = self.manager.terminate_session(&id).await;
                    // Adjust selection if needed
                    let count = self.manager.session_count();
                    if self.selected >= count && count > 0 {
                        self.selected = count - 1;
                    }
                }
                Ok(MenuAction::Continue)
            }
            
            _ => Ok(MenuAction::Continue)
        }
    }

    fn print_colored(&self, text: &str, color: Color) {
        let _ = execute!(io::stdout(), SetForegroundColor(color), Print(text), ResetColor);
    }
}

enum MenuAction {
    SelectSession(String),
    Quit,
    Continue,
}
