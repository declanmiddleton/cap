use anyhow::Result;
use colored::Colorize;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io::{self, stdout};
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{error, info};

use super::session::{ShellSessionManager, ShellState};

enum AppMode {
    Interactive,
    Menu,
}

pub struct InteractiveTerminal {
    manager: Arc<ShellSessionManager>,
    mode: AppMode,
    menu_state: ListState,
    command_buffer: String,
    output_lines: Vec<String>,
}

impl InteractiveTerminal {
    pub fn new(manager: Arc<ShellSessionManager>) -> Self {
        let mut menu_state = ListState::default();
        menu_state.select(Some(0));

        Self {
            manager,
            mode: AppMode::Interactive,
            menu_state,
            command_buffer: String::new(),
            output_lines: Vec::new(),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.run_loop(&mut terminal).await;

        // Restore terminal
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        result
    }

    async fn run_loop<B: ratatui::backend::Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<()> {
        let mut output_interval = interval(Duration::from_millis(100));

        loop {
            tokio::select! {
                _ = output_interval.tick() => {
                    // Update output from active session
                    if let Some(session) = self.manager.get_active_session().await {
                        let mut output_rx = session.output_rx.write().await;
                        while let Ok(line) = output_rx.try_recv() {
                            self.output_lines.push(line);
                            
                            // Keep only last 1000 lines
                            if self.output_lines.len() > 1000 {
                                self.output_lines.drain(0..100);
                            }
                        }
                    }

                    terminal.draw(|f| self.draw(f))?;
                }

                _ = async {
                    if event::poll(Duration::from_millis(50)).unwrap_or(false) {
                        match event::read() {
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

            // Check if we should exit
            if self.should_exit().await {
                break;
            }
        }

        Ok(())
    }

    async fn handle_key_event(&mut self, key: KeyEvent) -> Result<()> {
        match self.mode {
            AppMode::Interactive => self.handle_interactive_mode(key).await,
            AppMode::Menu => self.handle_menu_mode(key).await,
        }
    }

    async fn handle_interactive_mode(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            // F12 to open menu
            KeyCode::F(12) => {
                self.mode = AppMode::Menu;
                info!("Opened control menu");
            }
            // Ctrl+C to exit
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                info!("Exiting interactive terminal");
                std::process::exit(0);
            }
            // Enter to send command
            KeyCode::Enter => {
                if !self.command_buffer.is_empty() {
                    if let Some(session) = self.manager.get_active_session().await {
                        session.send_command(self.command_buffer.clone()).await?;
                        self.command_buffer.clear();
                    }
                }
            }
            // Backspace
            KeyCode::Backspace => {
                self.command_buffer.pop();
            }
            // Regular character input
            KeyCode::Char(c) => {
                self.command_buffer.push(c);
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_menu_mode(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            // Escape or F12 to close menu
            KeyCode::Esc | KeyCode::F(12) => {
                self.mode = AppMode::Interactive;
                info!("Closed control menu");
            }
            // Arrow keys for navigation
            KeyCode::Up => {
                self.menu_previous();
            }
            KeyCode::Down => {
                self.menu_next();
            }
            // Enter to execute menu action
            KeyCode::Enter => {
                self.execute_menu_action().await?;
            }
            _ => {}
        }
        Ok(())
    }

    fn menu_next(&mut self) {
        let sessions = self.manager.list_sessions();
        // 7 static menu items + 2 headers/dividers + sessions
        let menu_items = if sessions.is_empty() {
            10 // All menu items + "no sessions" message
        } else {
            9 + sessions.len() // Menu items + session header + sessions
        };
        
        let i = match self.menu_state.selected() {
            Some(i) => {
                if i >= menu_items - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.menu_state.select(Some(i));
    }

    fn menu_previous(&mut self) {
        let sessions = self.manager.list_sessions();
        let menu_items = if sessions.is_empty() {
            10
        } else {
            9 + sessions.len()
        };
        
        let i = match self.menu_state.selected() {
            Some(i) => {
                if i == 0 {
                    menu_items - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.menu_state.select(Some(i));
    }

    async fn execute_menu_action(&mut self) -> Result<()> {
        let selected = self.menu_state.selected().unwrap_or(0);
        let sessions = self.manager.list_sessions();

        if selected == 0 {
            // Close menu
            self.mode = AppMode::Interactive;
        } else if selected == 1 {
            // Exit terminal (listener keeps running)
            info!("Exiting interactive terminal, listener continues in background");
            std::process::exit(0);
        } else if selected == 2 {
            // Background current session
            if let Some(session) = self.manager.get_active_session().await {
                self.manager.background_session(&session.id).await?;
                self.mode = AppMode::Interactive;
            }
        } else if selected == 3 {
            // Terminate active session
            if let Some(session) = self.manager.get_active_session().await {
                self.manager.terminate_session(&session.id).await?;
                self.mode = AppMode::Interactive;
            }
        } else if selected == 4 {
            // Cleanup terminated
            self.manager.cleanup_terminated_sessions().await;
            self.mode = AppMode::Interactive;
        } else if selected == 5 {
            // Stop listener and exit
            info!("Stopping listener and exiting");
            std::process::exit(0);
        } else {
            // Switch to specific session
            let session_index = selected - 6;
            if session_index < sessions.len() {
                let (session_id, state) = &sessions[session_index];
                if *state != ShellState::Terminated {
                    self.manager.foreground_session(session_id).await?;
                    self.output_lines.clear();
                    self.mode = AppMode::Interactive;
                }
            }
        }

        Ok(())
    }

    fn draw(&mut self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),      // Header
                Constraint::Min(0),         // Main content
                Constraint::Length(3),      // Input
            ])
            .split(f.area());

        // Draw header
        self.draw_header(f, chunks[0]);

        // Draw main content
        match self.mode {
            AppMode::Interactive => self.draw_interactive(f, chunks[1]),
            AppMode::Menu => self.draw_menu(f, chunks[1]),
        }

        // Draw input
        self.draw_input(f, chunks[2]);
    }

    fn draw_header(&self, f: &mut Frame, area: Rect) {
        let active_session = futures::executor::block_on(self.manager.get_active_session());
        
        let header_text = if let Some(session) = active_session {
            format!(
                "CAP Shell Handler | Session: {} | Remote: {} | Connected: {}",
                &session.id[..8],
                session.metadata.remote_addr,
                session.metadata.connected_at.format("%H:%M:%S")
            )
        } else {
            "CAP Shell Handler | No active session | Press F12 for menu".to_string()
        };

        let header = Paragraph::new(header_text)
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .block(Block::default().borders(Borders::ALL).title("Status"))
            .alignment(Alignment::Center);

        f.render_widget(header, area);
    }

    fn draw_interactive(&self, f: &mut Frame, area: Rect) {
        let output_text = if self.output_lines.is_empty() {
            "Shell output will appear here...\n\nPress F12 to open control menu".to_string()
        } else {
            // Show last 50 lines
            let start = if self.output_lines.len() > 50 {
                self.output_lines.len() - 50
            } else {
                0
            };
            self.output_lines[start..].join("")
        };

        let output = Paragraph::new(output_text)
            .style(Style::default().fg(Color::White))
            .block(Block::default().borders(Borders::ALL).title("Shell Output"))
            .wrap(Wrap { trim: false });

        f.render_widget(output, area);
    }

    fn draw_menu(&mut self, f: &mut Frame, area: Rect) {
        let sessions = self.manager.list_sessions();
        
        let mut items = vec![
            ListItem::new("[ Close Menu (Return to Shell) ]").style(Style::default().fg(Color::Cyan)),
            ListItem::new("[ Exit Terminal (Listener Keeps Running) ]").style(Style::default().fg(Color::Green)),
            ListItem::new("---").style(Style::default().fg(Color::DarkGray)),
            ListItem::new("Background Current Session (Ctrl+Z)").style(Style::default().fg(Color::Yellow)),
            ListItem::new("Kill Active Session").style(Style::default().fg(Color::Red)),
            ListItem::new("Cleanup Terminated Sessions").style(Style::default().fg(Color::Magenta)),
            ListItem::new("[ Stop Listener & Exit ]").style(Style::default().fg(Color::Red)),
        ];

        if !sessions.is_empty() {
            items.push(ListItem::new("---").style(Style::default().fg(Color::DarkGray)));
            items.push(
                ListItem::new("Switch to Session:")
                    .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            );

            for (id, state) in &sessions {
                let state_str = match state {
                    ShellState::Active => "●".green().to_string(),
                    ShellState::Background => "◐".yellow().to_string(),
                    ShellState::Terminated => "○".red().to_string(),
                };
                let item_text = format!("{} {} ", state_str, &id[..12]);
                items.push(ListItem::new(item_text));
            }
        } else {
            items.push(ListItem::new("---").style(Style::default().fg(Color::DarkGray)));
            items.push(
                ListItem::new("[ No active sessions - waiting for connections... ]")
                    .style(Style::default().fg(Color::DarkGray)),
            );
        }

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Control Menu (F12 or ESC to close) - Penelope Style"),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        f.render_stateful_widget(list, area, &mut self.menu_state);
    }

    fn draw_input(&self, f: &mut Frame, area: Rect) {
        let input_text = format!("> {}", self.command_buffer);
        let input = Paragraph::new(input_text)
            .style(Style::default().fg(Color::Green))
            .block(Block::default().borders(Borders::ALL).title("Command"));

        f.render_widget(input, area);
    }

    async fn should_exit(&self) -> bool {
        // Could implement exit conditions here
        false
    }
}

