use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::Arc;

use super::scope::ScopeManager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub server: ServerConfig,
    #[serde(skip)]
    pub scope: Arc<ScopeManager>,
    pub audit: AuditConfig,
    pub modules: ModulesConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub tls_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditConfig {
    pub log_path: String,
    pub retention_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModulesConfig {
    pub default_threads: usize,
    pub timeout_seconds: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct ConfigFile {
    general: GeneralConfig,
    server: ServerConfig,
    scope: ScopeConfig,
    audit: AuditConfig,
    modules: ModulesConfig,
}

#[derive(Debug, Serialize, Deserialize)]
struct ScopeConfig {
    authorized_targets: Vec<String>,
}

impl Config {
    /// Load config from file, or use defaults if file doesn't exist
    pub fn load_or_default<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        
        // If config file doesn't exist, use defaults
        if !path.exists() {
            return Ok(Self::default());
        }
        
        // Try to load the config file
        Self::load(path)
    }

    /// Load config from file (fails if file doesn't exist)
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config_file: ConfigFile =
            toml::from_str(&contents).context("Failed to parse config file")?;

        let scope = Arc::new(ScopeManager::new(config_file.scope.authorized_targets));

        Ok(Config {
            general: config_file.general,
            server: config_file.server,
            scope,
            audit: config_file.audit,
            modules: config_file.modules,
        })
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();

        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        let config_file = ConfigFile {
            general: self.general.clone(),
            server: self.server.clone(),
            scope: ScopeConfig {
                authorized_targets: self.scope.list_targets(),
            },
            audit: self.audit.clone(),
            modules: self.modules.clone(),
        };

        let contents = toml::to_string_pretty(&config_file).context("Failed to serialize config")?;

        fs::write(path, contents)
            .with_context(|| format!("Failed to write config file: {}", path.display()))?;

        Ok(())
    }
}

impl Default for ScopeManager {
    fn default() -> Self {
        ScopeManager::new(vec![])
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            general: GeneralConfig {
                name: "CAP Project".to_string(),
                description: "Security assessment project".to_string(),
            },
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8443,
                tls_enabled: false,
            },
            scope: Arc::new(ScopeManager::new(vec![])),
            audit: AuditConfig {
                log_path: "logs/audit.jsonl".to_string(),
                retention_days: 90,
            },
            modules: ModulesConfig {
                default_threads: 10,
                timeout_seconds: 300,
            },
        }
    }
}

