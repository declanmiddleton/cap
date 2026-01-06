use anyhow::Result;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info};

use super::session::ShellSessionManager;

pub struct ShellListener {
    manager: Arc<ShellSessionManager>,
}

impl ShellListener {
    pub fn new(manager: Arc<ShellSessionManager>) -> Self {
        Self { manager }
    }

    pub async fn start(&self, host: &str, port: u16) -> Result<()> {
        let addr = format!("{}:{}", host, port);
        let listener = TcpListener::bind(&addr).await?;

        info!("Shell listener bound to {}", addr);

        loop {
            match listener.accept().await {
                Ok((stream, remote_addr)) => {
                    let manager = self.manager.clone();
                    
                    tokio::spawn(async move {
                        match manager.register_session(remote_addr, stream).await {
                            Ok(_session_id) => {
                                // Session manager will notify via terminal
                            }
                            Err(e) => {
                                error!("Failed to register shell session: {}", e);
                            }
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }
    }

    pub async fn start_with_cleanup(&self, host: &str, port: u16) -> Result<()> {
        let manager = self.manager.clone();
        
        // Spawn cleanup task
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                manager.cleanup_terminated_sessions().await;
            }
        });

        self.start(host, port).await
    }
}

