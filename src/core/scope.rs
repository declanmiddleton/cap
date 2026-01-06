use anyhow::Result;
use dashmap::DashSet;
use ipnetwork::IpNetwork;
use regex::Regex;
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;

/// ScopeManager enforces authorized target restrictions
/// This is a critical security feature to prevent unauthorized testing
#[derive(Debug)]
pub struct ScopeManager {
    // Authorized IP addresses and networks
    ip_ranges: Arc<DashSet<IpNetwork>>,
    // Authorized domain patterns (supports wildcards)
    domains: Arc<DashSet<String>>,
    // Authorized hostnames
    hosts: Arc<DashSet<String>>,
}

impl ScopeManager {
    pub fn new(initial_targets: Vec<String>) -> Self {
        let manager = Self {
            ip_ranges: Arc::new(DashSet::new()),
            domains: Arc::new(DashSet::new()),
            hosts: Arc::new(DashSet::new()),
        };

        for target in initial_targets {
            let _ = manager.add_target(&target);
        }

        manager
    }

    /// Add a target to the authorized scope
    /// Supports:
    /// - IP addresses: 192.168.1.1
    /// - CIDR ranges: 192.168.1.0/24
    /// - Domains: example.com, *.example.com
    /// - Hostnames: test.example.com
    pub fn add_target(&self, target: &str) -> Result<()> {
        let target = target.trim();

        // Try parsing as IP address
        if let Ok(ip) = IpAddr::from_str(target) {
            let network = match ip {
                IpAddr::V4(ipv4) => IpNetwork::V4(ipv4.into()),
                IpAddr::V6(ipv6) => IpNetwork::V6(ipv6.into()),
            };
            self.ip_ranges.insert(network);
            tracing::info!("Added IP address to scope: {}", target);
            return Ok(());
        }

        // Try parsing as CIDR network
        if let Ok(network) = IpNetwork::from_str(target) {
            self.ip_ranges.insert(network);
            tracing::info!("Added IP range to scope: {}", target);
            return Ok(());
        }

        // Otherwise treat as domain/hostname
        if target.contains('*') {
            self.domains.insert(target.to_string());
            tracing::info!("Added domain pattern to scope: {}", target);
        } else {
            self.hosts.insert(target.to_string());
            tracing::info!("Added hostname to scope: {}", target);
        }

        Ok(())
    }

    /// Remove a target from authorized scope
    pub fn remove_target(&self, target: &str) -> Result<()> {
        let target = target.trim();

        if let Ok(ip) = IpAddr::from_str(target) {
            let network = match ip {
                IpAddr::V4(ipv4) => IpNetwork::V4(ipv4.into()),
                IpAddr::V6(ipv6) => IpNetwork::V6(ipv6.into()),
            };
            self.ip_ranges.remove(&network);
        } else if let Ok(network) = IpNetwork::from_str(target) {
            self.ip_ranges.remove(&network);
        } else if target.contains('*') {
            self.domains.remove(target);
        } else {
            self.hosts.remove(target);
        }

        tracing::info!("Removed target from scope: {}", target);
        Ok(())
    }

    /// Check if a target is within authorized scope
    pub fn is_in_scope(&self, target: &str) -> bool {
        let target = target.trim();

        // Check IP address
        if let Ok(ip) = IpAddr::from_str(target) {
            for network in self.ip_ranges.iter() {
                if network.contains(ip) {
                    return true;
                }
            }
            return false;
        }

        // Check exact hostname match
        if self.hosts.contains(target) {
            return true;
        }

        // Check domain patterns (wildcards)
        for domain_pattern in self.domains.iter() {
            if self.matches_pattern(target, domain_pattern.as_str()) {
                return true;
            }
        }

        false
    }

    /// Match target against wildcard domain pattern
    fn matches_pattern(&self, target: &str, pattern: &str) -> bool {
        let regex_pattern = pattern
            .replace(".", "\\.")
            .replace("*", ".*");
        
        if let Ok(regex) = Regex::new(&format!("^{}$", regex_pattern)) {
            return regex.is_match(target);
        }

        false
    }

    /// List all authorized targets
    pub fn list_targets(&self) -> Vec<String> {
        let mut targets = Vec::new();

        for ip_range in self.ip_ranges.iter() {
            targets.push(ip_range.to_string());
        }

        for domain in self.domains.iter() {
            targets.push(domain.clone());
        }

        for host in self.hosts.iter() {
            targets.push(host.clone());
        }

        targets.sort();
        targets
    }

    /// Clear all authorized targets (use with caution)
    pub fn clear(&self) {
        self.ip_ranges.clear();
        self.domains.clear();
        self.hosts.clear();
        tracing::warn!("All targets cleared from scope");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ip_scope() {
        let scope = ScopeManager::new(vec!["192.168.1.0/24".to_string()]);
        assert!(scope.is_in_scope("192.168.1.1"));
        assert!(scope.is_in_scope("192.168.1.254"));
        assert!(!scope.is_in_scope("192.168.2.1"));
    }

    #[test]
    fn test_domain_wildcard() {
        let scope = ScopeManager::new(vec!["*.example.com".to_string()]);
        assert!(scope.is_in_scope("test.example.com"));
        assert!(scope.is_in_scope("api.example.com"));
        assert!(!scope.is_in_scope("example.com"));
        assert!(!scope.is_in_scope("evil.com"));
    }

    #[test]
    fn test_hostname_scope() {
        let scope = ScopeManager::new(vec!["test.example.com".to_string()]);
        assert!(scope.is_in_scope("test.example.com"));
        assert!(!scope.is_in_scope("prod.example.com"));
    }
}

