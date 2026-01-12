use anyhow::Result;
use colored::Colorize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::net::TcpListener;
use tracing::{error, info};

use super::session::ShellSessionManager;

pub struct ShellListener {
    manager: Arc<ShellSessionManager>,
    should_stop: Arc<AtomicBool>,
}

impl ShellListener {
    pub fn new(manager: Arc<ShellSessionManager>) -> Self {
        Self { 
            manager,
            should_stop: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn start(&self, host: &str, port: u16) -> Result<()> {
        let addr = format!("{}:{}", host, port);
        let listener = TcpListener::bind(&addr).await?;

        // Setup Ctrl+C handler
        let should_stop_clone = self.should_stop.clone();
        let port_clone = port;
        tokio::spawn(async move {
            if let Ok(()) = tokio::signal::ctrl_c().await {
                println!();
                println!("{}", "Ctrl+C received. Shutting down listener...".bright_yellow());
                should_stop_clone.store(true, Ordering::Relaxed);
                
                // Give time for cleanup
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                
                println!("{}", format!("Listener on port {} closed.", port_clone).truecolor(120, 120, 130));
                std::process::exit(0);
            }
        });

        info!("Shell listener bound to {}", addr);

        loop {
            // Check if we should stop
            if self.should_stop.load(Ordering::Relaxed) {
                break;
            }

            // Use a timeout on accept to allow periodic should_stop checks
            tokio::select! {
                result = listener.accept() => {
                    match result {
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
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                    // Periodic check for should_stop
                    continue;
                }
            }
        }

        Ok(())
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

    pub fn stop(&self) {
        self.should_stop.store(true, Ordering::Relaxed);
    }
}
