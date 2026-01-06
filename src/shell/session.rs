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

#[derive(Debug, Clone)]
pub struct ShellMetadata {
    pub remote_addr: String,
    pub hostname: Option<String>,
    pub username: Option<String>,
    pub os_type: Option<String>,
    pub connected_at: DateTime<Utc>,
}

pub struct ShellSession {
    pub id: String,
    pub name: String,
    pub metadata: ShellMetadata,
    pub state: Arc<RwLock<ShellState>>,
    pub stream: Arc<RwLock<Option<TcpStream>>>,
    pub output_buffer: Arc<RwLock<Vec<String>>>,
    pub input_tx: mpsc::UnboundedSender<String>,
    pub output_rx: Arc<RwLock<mpsc::UnboundedReceiver<String>>>,
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
            connected_at: Utc::now(),
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
            metadata,
            state,
            stream: stream_arc,
            output_buffer,
            input_tx,
            output_rx: Arc::new(RwLock::new(output_rx)),
        })
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
                            info!("Shell session {} disconnected", id);
                            *state.write().await = ShellState::Terminated;
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
                            *state.write().await = ShellState::Terminated;
                            break;
                        }
                    }
                }
                
                // Handle outgoing commands to shell
                Some(command) = input_rx.recv() => {
                    if let Err(e) = writer.write_all(command.as_bytes()).await {
                        error!("Write error on shell session {}: {}", id, e);
                        *state.write().await = ShellState::Terminated;
                        break;
                    }
                    if let Err(e) = writer.flush().await {
                        error!("Flush error on shell session {}: {}", id, e);
                        *state.write().await = ShellState::Terminated;
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
        
        info!(
            "🐚 New shell session registered: {} from {}",
            session_id, remote_addr
        );

        let session_arc = Arc::new(session);
        self.sessions.insert(session_id.clone(), session_arc);

        // Set as active if no active session
        let mut active = self.active_session.write().await;
        if active.is_none() {
            *active = Some(session_id.clone());
            info!("Set session {} as active", session_id);
        }

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
        }
        
        let mut sessions_info = Vec::new();
        for entry in self.sessions.iter() {
            let state = entry.value().get_state().await;
            let state_str = match state {
                ShellState::Active => "Active",
                ShellState::Background => "Background",
                ShellState::Terminated => "Terminated",
            };
            
            sessions_info.push(SessionInfo {
                id: entry.key().clone(),
                remote_addr: entry.value().metadata.remote_addr.clone(),
                state: state_str.to_string(),
                connected_at: entry.value().metadata.connected_at.to_rfc3339(),
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
}

impl Default for ShellSessionManager {
    fn default() -> Self {
        Self::new()
    }
}

