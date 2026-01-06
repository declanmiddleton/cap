use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// Immutable audit log entry
/// Each entry is cryptographically linked to previous entries for integrity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub timestamp: DateTime<Utc>,
    pub session_id: Option<String>,
    pub event_type: String,
    pub description: String,
    pub operator: String,
    pub target: Option<String>,
    pub result: Option<String>,
    pub previous_hash: String,
    pub current_hash: String,
}

pub struct AuditLogger {
    log_path: PathBuf,
}

impl AuditLogger {
    pub fn new<P: AsRef<Path>>(log_path: P) -> Result<Self> {
        let log_path = log_path.as_ref().to_path_buf();
        
        // Create parent directories if they don't exist
        if let Some(parent) = log_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        Ok(Self { log_path })
    }

    /// Log an event with cryptographic integrity
    pub fn log(
        &self,
        session_id: Option<&str>,
        event_type: &str,
        description: &str,
        target: Option<&str>,
        result: Option<&str>,
    ) -> Result<()> {
        let operator = whoami::username();
        let previous_hash = self.get_last_hash()?;

        let entry = AuditLogEntry {
            timestamp: Utc::now(),
            session_id: session_id.map(|s| s.to_string()),
            event_type: event_type.to_string(),
            description: description.to_string(),
            operator,
            target: target.map(|t| t.to_string()),
            result: result.map(|r| r.to_string()),
            previous_hash: previous_hash.clone(),
            current_hash: String::new(), // Will be computed
        };

        let entry = self.compute_hash(entry);

        // Append to log file (JSONL format)
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .context("Failed to open audit log file")?;

        let json = serde_json::to_string(&entry).context("Failed to serialize audit entry")?;
        writeln!(file, "{}", json).context("Failed to write audit entry")?;

        tracing::debug!(
            "Audit log: {} - {} (hash: {})",
            event_type,
            description,
            &entry.current_hash[..8]
        );

        Ok(())
    }

    /// Compute SHA-256 hash of audit entry
    fn compute_hash(&self, mut entry: AuditLogEntry) -> AuditLogEntry {
        let hash_input = format!(
            "{}|{}|{}|{}|{}|{}|{}",
            entry.timestamp.to_rfc3339(),
            entry.session_id.as_deref().unwrap_or(""),
            entry.event_type,
            entry.description,
            entry.operator,
            entry.target.as_deref().unwrap_or(""),
            entry.previous_hash
        );

        let mut hasher = Sha256::new();
        hasher.update(hash_input.as_bytes());
        let result = hasher.finalize();
        entry.current_hash = format!("{:x}", result);
        entry
    }

    /// Get the hash of the last log entry (for chain integrity)
    fn get_last_hash(&self) -> Result<String> {
        if !self.log_path.exists() {
            return Ok("genesis".to_string());
        }

        let file = File::open(&self.log_path)?;
        let reader = BufReader::new(file);
        
        if let Some(last_line) = reader.lines().last() {
            let last_line = last_line?;
            let entry: AuditLogEntry = serde_json::from_str(&last_line)?;
            return Ok(entry.current_hash);
        }

        Ok("genesis".to_string())
    }

    /// Read audit logs (optionally filtered by session)
    pub fn read_logs(&self, session_id: Option<&str>) -> Result<Vec<AuditLogEntry>> {
        if !self.log_path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&self.log_path)?;
        let reader = BufReader::new(file);
        let mut logs = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if let Ok(entry) = serde_json::from_str::<AuditLogEntry>(&line) {
                if let Some(filter_session) = session_id {
                    if entry.session_id.as_deref() == Some(filter_session) {
                        logs.push(entry);
                    }
                } else {
                    logs.push(entry);
                }
            }
        }

        Ok(logs)
    }

    /// Verify the integrity of the audit log chain
    pub fn verify_integrity(&self) -> Result<bool> {
        let logs = self.read_logs(None)?;
        let mut expected_previous_hash = "genesis".to_string();

        for entry in logs {
            // Check if previous hash matches
            if entry.previous_hash != expected_previous_hash {
                tracing::error!(
                    "Audit log integrity violation: expected previous hash '{}', got '{}'",
                    expected_previous_hash,
                    entry.previous_hash
                );
                return Ok(false);
            }

            // Recompute hash to verify current hash
            let mut entry_for_hash = entry.clone();
            entry_for_hash.current_hash = String::new();
            let recomputed_entry = self.compute_hash(entry_for_hash);

            if recomputed_entry.current_hash != entry.current_hash {
                tracing::error!(
                    "Audit log integrity violation: hash mismatch for entry at {}",
                    entry.timestamp
                );
                return Ok(false);
            }

            expected_previous_hash = entry.current_hash;
        }

        Ok(true)
    }

    /// Export logs to a file
    pub fn export_logs(&self, logs: &[AuditLogEntry], output_path: &str) -> Result<()> {
        let json = serde_json::to_string_pretty(logs)?;
        std::fs::write(output_path, json)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_audit_log_integrity() {
        let temp_file = NamedTempFile::new().unwrap();
        let logger = AuditLogger::new(temp_file.path()).unwrap();

        logger
            .log(
                Some("session-123"),
                "module_execution",
                "Executed web enumeration",
                Some("example.com"),
                Some("success"),
            )
            .unwrap();

        logger
            .log(
                Some("session-123"),
                "module_execution",
                "Executed DNS enumeration",
                Some("example.com"),
                Some("success"),
            )
            .unwrap();

        assert!(logger.verify_integrity().unwrap());
    }
}

