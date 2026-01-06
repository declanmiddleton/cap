pub mod dns_enum;
pub mod port_scan;
pub mod web_enum;

use anyhow::{Context, Result};
use async_trait::async_trait;
use std::sync::Arc;

use crate::core::{audit::AuditLogger, config::Config, session::SessionManager};

#[async_trait]
pub trait SecurityModule: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    async fn execute(
        &self,
        target: &str,
        config: &ModuleConfig,
    ) -> Result<Vec<String>>;
}

#[derive(Debug, Clone)]
pub struct ModuleConfig {
    pub wordlist: Option<String>,
    pub threads: usize,
    pub timeout_seconds: u64,
    pub verbose: bool,
    pub status_codes: Option<Vec<u16>>,
    pub exclude_codes: Option<Vec<u16>>,
}

pub struct ModuleExecutor {
    config: Config,
    session_manager: SessionManager,
}

impl ModuleExecutor {
    pub fn new(config: Config, session_manager: SessionManager) -> Self {
        Self {
            config,
            session_manager,
        }
    }

    pub async fn execute(
        &self,
        module_name: &str,
        target: &str,
        wordlist: Option<String>,
        threads: usize,
    ) -> Result<Vec<String>> {
        self.execute_with_options(
            module_name,
            target,
            wordlist,
            threads,
            false,
            None,
            None,
        )
        .await
    }

    pub async fn execute_with_options(
        &self,
        module_name: &str,
        target: &str,
        wordlist: Option<String>,
        threads: usize,
        verbose: bool,
        status_codes: Option<Vec<u16>>,
        exclude_codes: Option<Vec<u16>>,
    ) -> Result<Vec<String>> {
        // Create audit logger
        let audit_logger = AuditLogger::new(&self.config.audit.log_path)?;

        // Log module execution start
        audit_logger.log(
            None,
            "module_execution_start",
            &format!("Starting {} module", module_name),
            Some(target),
            None,
        )?;

        let module_config = ModuleConfig {
            wordlist,
            threads,
            timeout_seconds: self.config.modules.timeout_seconds,
            verbose,
            status_codes,
            exclude_codes,
        };

        let module: Arc<dyn SecurityModule> = match module_name {
            "web-enum" | "web" => Arc::new(web_enum::WebEnumerationModule::new()),
            "dns-enum" | "dns" => Arc::new(dns_enum::DnsEnumerationModule::new()),
            "port-scan" | "ports" => Arc::new(port_scan::PortScanModule::new()),
            _ => {
                anyhow::bail!("Unknown module: {}", module_name);
            }
        };

        tracing::info!(
            "Executing module '{}' against target '{}'",
            module.name(),
            target
        );

        let results = module
            .execute(target, &module_config)
            .await
            .context("Module execution failed")?;

        // Log module execution completion
        audit_logger.log(
            None,
            "module_execution_complete",
            &format!("Completed {} module", module_name),
            Some(target),
            Some(&format!("{} results", results.len())),
        )?;

        Ok(results)
    }

    pub fn list_modules(&self) -> Vec<(&str, &str)> {
        vec![
            ("web-enum", "Web application enumeration using wordlists"),
            ("dns-enum", "DNS and subdomain enumeration"),
            ("port-scan", "Network port scanning"),
        ]
    }
}

