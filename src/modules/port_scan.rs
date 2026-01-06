use anyhow::Result;
use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;

use super::{ModuleConfig, SecurityModule};

pub struct PortScanModule;

impl PortScanModule {
    pub fn new() -> Self {
        Self
    }

    async fn check_port(&self, ip: IpAddr, port: u16, timeout_duration: Duration) -> Option<String> {
        let socket_addr = SocketAddr::new(ip, port);
        
        match timeout(timeout_duration, TcpStream::connect(socket_addr)).await {
            Ok(Ok(_)) => {
                let service = self.get_service_name(port);
                Some(format!("{}:{} - OPEN ({})", ip, port, service))
            }
            _ => None,
        }
    }

    fn get_service_name(&self, port: u16) -> &str {
        match port {
            21 => "FTP",
            22 => "SSH",
            23 => "Telnet",
            25 => "SMTP",
            53 => "DNS",
            80 => "HTTP",
            110 => "POP3",
            143 => "IMAP",
            443 => "HTTPS",
            445 => "SMB",
            3306 => "MySQL",
            3389 => "RDP",
            5432 => "PostgreSQL",
            5900 => "VNC",
            6379 => "Redis",
            8080 => "HTTP-Proxy",
            8443 => "HTTPS-Alt",
            9200 => "Elasticsearch",
            27017 => "MongoDB",
            _ => "Unknown",
        }
    }

    fn get_common_ports() -> Vec<u16> {
        vec![
            21, 22, 23, 25, 53, 80, 110, 143, 443, 445, 465, 587, 993, 995,
            1433, 3306, 3389, 5432, 5900, 6379, 8000, 8080, 8443, 9200, 27017,
        ]
    }
}

#[async_trait]
impl SecurityModule for PortScanModule {
    fn name(&self) -> &str {
        "port-scan"
    }

    fn description(&self) -> &str {
        "Network port scanning to identify open ports and services"
    }

    async fn execute(
        &self,
        target: &str,
        config: &ModuleConfig,
    ) -> Result<Vec<String>> {
        tracing::info!("Starting port scan for {}", target);

        // Parse target as IP address
        let ip = if let Ok(addr) = IpAddr::from_str(target) {
            addr
        } else {
            // Try resolving as hostname
            let resolver = trust_dns_resolver::TokioAsyncResolver::tokio(
                trust_dns_resolver::config::ResolverConfig::default(),
                trust_dns_resolver::config::ResolverOpts::default(),
            );

            let lookup = resolver.lookup_ip(target).await?;
            lookup
                .iter()
                .next()
                .ok_or_else(|| anyhow::anyhow!("Could not resolve hostname: {}", target))?
        };

        // Use common ports for scanning
        let ports = Self::get_common_ports();

        tracing::info!(
            "Scanning {} ports on {} with {} threads",
            ports.len(),
            ip,
            config.threads
        );

        let timeout_duration = Duration::from_secs(2);
        let results = stream::iter(ports)
            .map(|port| async move {
                self.check_port(ip, port, timeout_duration).await
            })
            .buffer_unordered(config.threads)
            .filter_map(|result| async move { result })
            .collect::<Vec<_>>()
            .await;

        tracing::info!("Port scan completed: {} open ports found", results.len());

        Ok(results)
    }
}

