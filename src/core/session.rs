use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use super::config::Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub status: SessionStatus,
    pub metadata: SessionMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SessionStatus {
    Active,
    Paused,
    Terminated,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub operator: String,
    pub purpose: String,
    pub authorization_ref: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Clone)]
pub struct SessionManager {
    sessions: Arc<DashMap<String, Session>>,
    config: Config,
}

impl SessionManager {
    pub fn new(config: Config) -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            config,
        }
    }

    /// Create a new session with automatic expiration
    pub async fn create_session(&self, name: String) -> Result<Session> {
        let session_id = Uuid::new_v4().to_string();
        
        // Sessions automatically expire after 24 hours by default
        let expires_at = Some(Utc::now() + Duration::hours(24));

        let session = Session {
            id: session_id.clone(),
            name: name.clone(),
            created_at: Utc::now(),
            expires_at,
            status: SessionStatus::Active,
            metadata: SessionMetadata {
                operator: whoami::username(),
                purpose: String::new(),
                authorization_ref: None,
                tags: vec![],
            },
        };

        self.sessions.insert(session_id.clone(), session.clone());

        tracing::info!(
            "Created new session: {} ({})",
            session_id,
            name
        );

        Ok(session)
    }

    /// Get a session by ID
    pub async fn get_session(&self, id: &str) -> Option<Session> {
        self.sessions.get(id).map(|entry| entry.clone())
    }

    /// List all active sessions
    pub async fn list_sessions(&self) -> Vec<Session> {
        self.sessions
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Terminate a session
    pub async fn terminate_session(&self, id: &str) -> Result<()> {
        if let Some(mut session) = self.sessions.get_mut(id) {
            session.status = SessionStatus::Terminated;
            tracing::info!("Terminated session: {}", id);
            Ok(())
        } else {
            anyhow::bail!("Session not found: {}", id)
        }
    }

    /// Pause a session
    pub async fn pause_session(&self, id: &str) -> Result<()> {
        if let Some(mut session) = self.sessions.get_mut(id) {
            session.status = SessionStatus::Paused;
            tracing::info!("Paused session: {}", id);
            Ok(())
        } else {
            anyhow::bail!("Session not found: {}", id)
        }
    }

    /// Resume a paused session
    pub async fn resume_session(&self, id: &str) -> Result<()> {
        if let Some(mut session) = self.sessions.get_mut(id) {
            if session.status == SessionStatus::Paused {
                session.status = SessionStatus::Active;
                tracing::info!("Resumed session: {}", id);
                Ok(())
            } else {
                anyhow::bail!("Session is not paused: {}", id)
            }
        } else {
            anyhow::bail!("Session not found: {}", id)
        }
    }

    /// Check and expire old sessions
    pub async fn cleanup_expired_sessions(&self) {
        let now = Utc::now();
        let mut expired_sessions = Vec::new();

        for entry in self.sessions.iter() {
            let session = entry.value();
            if let Some(expires_at) = session.expires_at {
                if now > expires_at && session.status == SessionStatus::Active {
                    expired_sessions.push(session.id.clone());
                }
            }
        }

        for session_id in expired_sessions {
            if let Some(mut session) = self.sessions.get_mut(&session_id) {
                session.status = SessionStatus::Expired;
                tracing::info!("Session expired: {}", session_id);
            }
        }
    }

    /// Update session metadata
    pub async fn update_metadata(
        &self,
        id: &str,
        purpose: Option<String>,
        authorization_ref: Option<String>,
        tags: Option<Vec<String>>,
    ) -> Result<()> {
        if let Some(mut session) = self.sessions.get_mut(id) {
            if let Some(purpose) = purpose {
                session.metadata.purpose = purpose;
            }
            if let Some(auth_ref) = authorization_ref {
                session.metadata.authorization_ref = Some(auth_ref);
            }
            if let Some(tags) = tags {
                session.metadata.tags = tags;
            }
            tracing::info!("Updated metadata for session: {}", id);
            Ok(())
        } else {
            anyhow::bail!("Session not found: {}", id)
        }
    }
}

