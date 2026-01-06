use anyhow::{Context, Result};
use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use std::fs::File;
use std::io::{BufRead, BufReader};
use trust_dns_resolver::TokioAsyncResolver;
use trust_dns_resolver::config::*;

use super::{ModuleConfig, SecurityModule};

pub struct DnsEnumerationModule;

impl DnsEnumerationModule {
    pub fn new() -> Self {
        Self
    }

    async fn resolve_subdomain(
        &self,
        resolver: &TokioAsyncResolver,
        subdomain: &str,
        domain: &str,
    ) -> Option<String> {
        let full_domain = if subdomain.is_empty() {
            domain.to_string()
        } else {
            format!("{}.{}", subdomain, domain)
        };

        match resolver.lookup_ip(&full_domain).await {
            Ok(response) => {
                let ips: Vec<_> = response.iter().collect();
                if !ips.is_empty() {
                    Some(format!(
                        "{} -> {}",
                        full_domain,
                        ips.iter()
                            .map(|ip| ip.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    }
}

#[async_trait]
impl SecurityModule for DnsEnumerationModule {
    fn name(&self) -> &str {
        "dns-enum"
    }

    fn description(&self) -> &str {
        "DNS and subdomain enumeration to discover subdomains and DNS records"
    }

    async fn execute(
        &self,
        target: &str,
        config: &ModuleConfig,
    ) -> Result<Vec<String>> {
        tracing::info!("Starting DNS enumeration for {}", target);

        // Remove protocol if present
        let domain = target
            .trim_start_matches("http://")
            .trim_start_matches("https://")
            .split('/')
            .next()
            .unwrap_or(target);

        let wordlist_path = config
            .wordlist
            .clone()
            .unwrap_or_else(|| "wordlists/subdomains.txt".to_string());

        // Read subdomain wordlist
        let subdomains = if std::path::Path::new(&wordlist_path).exists() {
            let file = File::open(&wordlist_path)
                .context("Failed to open subdomain wordlist")?;
            let reader = BufReader::new(file);
            reader
                .lines()
                .filter_map(|line| line.ok())
                .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
                .collect::<Vec<_>>()
        } else {
            tracing::warn!("Subdomain wordlist not found, using defaults");
            vec![
                "www", "mail", "ftp", "admin", "api", "dev", "test", "staging",
                "beta", "app", "portal", "blog", "shop", "store", "vpn", "remote",
            ]
            .into_iter()
            .map(|s| s.to_string())
            .collect()
        };

        tracing::info!(
            "Testing {} subdomains with {} threads",
            subdomains.len(),
            config.threads
        );

        // Create resolver
        let resolver = TokioAsyncResolver::tokio(
            ResolverConfig::default(),
            ResolverOpts::default(),
        );

        let results = stream::iter(subdomains)
            .map(|subdomain| {
                let domain = domain.to_string();
                let resolver = resolver.clone();
                async move {
                    self.resolve_subdomain(&resolver, &subdomain, &domain).await
                }
            })
            .buffer_unordered(config.threads)
            .filter_map(|result| async move { result })
            .collect::<Vec<_>>()
            .await;

        tracing::info!("DNS enumeration completed: {} subdomains found", results.len());

        Ok(results)
    }
}

