use anyhow::{Context, Result};
use async_trait::async_trait;
use colored::Colorize;
use futures::stream::{self, StreamExt};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::{ModuleConfig, SecurityModule};

#[derive(Debug, Clone)]
pub struct WebEnumResult {
    pub url: String,
    pub status_code: u16,
    pub content_length: Option<u64>,
    pub method: String,
}

#[derive(Clone)]
pub struct WebEnumerationModule {
    methods: Vec<String>,
    status_codes: Vec<u16>,
    extensions: Vec<String>,
}

impl WebEnumerationModule {
    pub fn new() -> Self {
        Self {
            methods: vec!["GET".to_string()],
            status_codes: vec![200, 201, 204, 301, 302, 307, 401, 403],
            extensions: vec![],
        }
    }

    pub fn with_status_codes(mut self, codes: Vec<u16>) -> Self {
        self.status_codes = codes;
        self
    }

    pub fn exclude_status_codes(mut self, exclude: Vec<u16>) -> Self {
        self.status_codes.retain(|code| !exclude.contains(code));
        self
    }

    /// Discover wordlists in common locations
    pub fn discover_wordlists() -> Vec<PathBuf> {
        let mut wordlists = Vec::new();
        
        let search_paths = vec![
            "/usr/share/wordlists/",
            "/usr/share/wordlists/dirbuster/",
            "/usr/share/wordlists/dirb/",
            "/usr/share/seclists/Discovery/Web-Content/",
            "/snap/seclists/current/Discovery/Web-Content/",
            "wordlists/",
        ];

        for base_path in search_paths {
            if let Ok(entries) = fs::read_dir(base_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        if let Some(ext) = path.extension() {
                            if ext == "txt" {
                                wordlists.push(path);
                            }
                        }
                    }
                }
            }
        }

        wordlists
    }

    /// Find a specific wordlist by name
    pub fn find_wordlist(name: &str) -> Option<PathBuf> {
        let wordlists = Self::discover_wordlists();
        
        // Exact match
        if let Some(wl) = wordlists.iter().find(|w| {
            w.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n == name)
                .unwrap_or(false)
        }) {
            return Some(wl.clone());
        }

        // Partial match
        wordlists.into_iter().find(|w| {
            w.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.to_lowercase().contains(&name.to_lowercase()))
                .unwrap_or(false)
        })
    }

    /// Get default wordlist with fallback
    pub fn get_default_wordlist() -> PathBuf {
        // Try common wordlists in order of preference
        let preferred = vec![
            "common.txt",
            "directory-list-2.3-small.txt",
            "raft-small-words.txt",
            "big.txt",
        ];

        for name in preferred {
            if let Some(path) = Self::find_wordlist(name) {
                return path;
            }
        }

        // Fallback to local wordlist
        PathBuf::from("wordlists/common.txt")
    }

    /// Check a single path with HTTP method
    async fn check_path(
        &self,
        base_url: &str,
        path: &str,
        method: &str,
        timeout: Duration,
    ) -> Option<WebEnumResult> {
        let url = format!(
            "{}/{}",
            base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        );

        let client = reqwest::Client::builder()
            .timeout(timeout)
            .danger_accept_invalid_certs(true)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .ok()?;

        let request = match method {
            "GET" => client.get(&url),
            "POST" => client.post(&url),
            "HEAD" => client.head(&url),
            "PUT" => client.put(&url),
            "DELETE" => client.delete(&url),
            "OPTIONS" => client.request(reqwest::Method::OPTIONS, &url),
            _ => client.get(&url),
        };

        match request.send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                
                // Filter by status codes
                if self.status_codes.contains(&status) {
                    let content_length = response.content_length();
                    
                    Some(WebEnumResult {
                        url: url.clone(),
                        status_code: status,
                        content_length,
                        method: method.to_string(),
                    })
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    }

    /// Load wordlist from file
    fn load_wordlist<P: AsRef<Path>>(path: P) -> Result<Vec<String>> {
        let file = File::open(path.as_ref())
            .context(format!("Failed to open wordlist: {:?}", path.as_ref()))?;
        
        let reader = BufReader::new(file);
        let mut words = Vec::new();

        for line in reader.lines() {
            if let Ok(line) = line {
                let trimmed = line.trim();
                if !trimmed.is_empty() && !trimmed.starts_with('#') {
                    words.push(trimmed.to_string());
                }
            }
        }

        Ok(words)
    }

    /// Generate paths with extensions
    fn generate_paths_with_extensions(&self, base_paths: Vec<String>) -> Vec<String> {
        let mut all_paths = base_paths.clone();
        
        if !self.extensions.is_empty() {
            for path in &base_paths {
                for ext in &self.extensions {
                    all_paths.push(format!("{}.{}", path, ext));
                }
            }
        }
        
        all_paths
    }

    /// Display results in a formatted table
    pub fn display_results(results: &[WebEnumResult]) {
        if results.is_empty() {
            println!("\n{} No resources discovered", "ℹ".bright_blue());
            return;
        }

        println!("\n{}", "═══════════════════════════════════════════════════════════════".bright_blue());
        println!("{}", "                    DISCOVERED RESOURCES".bright_yellow().bold());
        println!("{}", "═══════════════════════════════════════════════════════════════\n".bright_blue());

        // Group by status code
        let mut by_status: std::collections::HashMap<u16, Vec<&WebEnumResult>> = std::collections::HashMap::new();
        for result in results {
            by_status.entry(result.status_code).or_insert_with(Vec::new).push(result);
        }

        for (status, items) in by_status.iter() {
            let status_color = match status {
                200..=299 => "green",
                300..=399 => "yellow",
                400..=499 => "red",
                _ => "white",
            };

            println!("{} {} ({})", 
                "●".color(status_color),
                format!("Status {}", status).bold(),
                format!("{} found", items.len()).bright_black()
            );
            
            for result in items {
                let size = if let Some(len) = result.content_length {
                    format!("{} bytes", len)
                } else {
                    "unknown".to_string()
                };

                println!("  {} {} {} {}",
                    format!("[{}]", result.method).bright_black(),
                    result.url.bright_white(),
                    format!("[{}]", result.status_code).color(status_color),
                    format!("({})", size).bright_black()
                );
            }
            println!();
        }

        println!("{}", "═══════════════════════════════════════════════════════════════".bright_blue());
        println!("\n{} Total: {} resources discovered\n", 
            "✓".green(),
            results.len().to_string().yellow()
        );
    }
}

#[async_trait]
impl SecurityModule for WebEnumerationModule {
    fn name(&self) -> &str {
        "web-enum"
    }

    fn description(&self) -> &str {
        "Web application enumeration using wordlists to discover directories and files"
    }

    async fn execute(
        &self,
        target: &str,
        config: &ModuleConfig,
    ) -> Result<Vec<String>> {
        let base_url = if target.starts_with("http://") || target.starts_with("https://") {
            target.to_string()
        } else {
            format!("https://{}", target)
        };

        // Configure status codes based on user input
        let mut module = self.clone();
        if let Some(ref codes) = config.status_codes {
            module.status_codes = codes.clone();
        }
        if let Some(ref exclude) = config.exclude_codes {
            module.status_codes.retain(|code| !exclude.contains(code));
        }

        println!("\n{}", "═══════════════════════════════════════════════════════════════".bright_blue());
        println!("{}", "                WEB ENUMERATION MODULE".bright_yellow().bold());
        println!("{}", "═══════════════════════════════════════════════════════════════\n".bright_blue());

        // Determine wordlist
        let wordlist_path = if let Some(ref wl) = config.wordlist {
            PathBuf::from(wl)
        } else {
            let default_wl = Self::get_default_wordlist();
            println!("{} Using wordlist: {}", 
                "→".blue(),
                default_wl.display().to_string().yellow()
            );
            default_wl
        };

        // Load wordlist
        let base_paths = Self::load_wordlist(&wordlist_path)
            .context("Failed to load wordlist")?;

        // Generate paths with extensions
        let all_paths = self.generate_paths_with_extensions(base_paths);

        println!("{} Target: {}", "→".blue(), base_url.yellow());
        println!("{} Wordlist: {}", "→".blue(), wordlist_path.display().to_string().yellow());
        println!("{} Total paths: {}", "→".blue(), all_paths.len().to_string().yellow());
        println!("{} Threads: {}", "→".blue(), config.threads.to_string().yellow());
        println!("{} Methods: {}", "→".blue(), module.methods.join(", ").yellow());
        println!("{} Status codes: {}", "→".blue(), 
            module.status_codes.iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(", ")
                .yellow()
        );
        
        if config.verbose {
            println!("{} {}", "→".blue(), "Verbose mode: ON (showing all responses)".bright_yellow());
        }
        println!();

        tracing::info!("Starting web enumeration for {} with {} paths", target, all_paths.len());

        let timeout = Duration::from_secs(config.timeout_seconds);
        let mut all_results = Vec::new();
        let verbose = config.verbose;

        // Test each method
        for method in &module.methods {
            println!("{} Testing {} method...", "⚡".yellow(), method.bright_white());
            println!();

            let mut tested = 0;
            let total = all_paths.len();
            
            let results: Vec<WebEnumResult> = stream::iter(all_paths.clone())
                .map(|path| {
                    let base_url = base_url.clone();
                    let method = method.clone();
                    let module_clone = module.clone();
                    async move {
                        let result = module_clone.check_path(&base_url, &path, &method, timeout).await;
                        
                        // In verbose mode, show all attempts
                        if verbose {
                            if let Some(ref res) = result {
                                let status_color = match res.status_code {
                                    200..=299 => "green",
                                    300..=399 => "yellow",
                                    400..=499 => "red",
                                    _ => "white",
                                };
                                println!("  {} {} {}",
                                    format!("[{}]", res.status_code).color(status_color),
                                    res.url.bright_white(),
                                    format!("({} bytes)", res.content_length.unwrap_or(0)).bright_black()
                                );
                            } else {
                                // Show failed attempts in verbose mode
                                let url = format!("{}/{}", base_url.trim_end_matches('/'), path.trim_start_matches('/'));
                                println!("  {} {}", "[---]".bright_black(), url.bright_black());
                            }
                        }
                        
                        result
                    }
                })
                .buffer_unordered(config.threads)
                .filter_map(|result| async move { result })
                .collect()
                .await;

            if !verbose && !results.is_empty() {
                println!("{} Found {} resources with {}", 
                    "✓".green(), 
                    results.len().to_string().yellow(),
                    method.bright_white()
                );
            } else if verbose {
                println!("\n{} Scan complete: {} matching resources found", 
                    "✓".green(), 
                    results.len().to_string().yellow()
                );
            }
            println!();

            all_results.extend(results);
        }

        // Display results
        Self::display_results(&all_results);

        tracing::info!("Web enumeration completed: {} resources found", all_results.len());

        // Convert to simple string format for compatibility
        let simple_results: Vec<String> = all_results
            .iter()
            .map(|r| format!("{} [{}] ({})", 
                r.url, 
                r.status_code,
                r.content_length.map(|l| format!("{} bytes", l)).unwrap_or_else(|| "unknown".to_string())
            ))
            .collect();

        Ok(simple_results)
    }
}
