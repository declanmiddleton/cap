use anyhow::Result;
use colored::Colorize;
use reqwest::Client;
use std::time::{Duration, Instant};

use super::request::{HttpRequest, InjectionPoint};
use super::analyzer::ResponseAnalysis;

#[derive(Debug, Clone)]
pub struct InjectionConfig {
    pub dry_run: bool,
    pub confirm_each: bool,
    pub timeout_secs: u64,
    pub follow_redirects: bool,
}

impl Default for InjectionConfig {
    fn default() -> Self {
        Self {
            dry_run: false,
            confirm_each: false,
            timeout_secs: 10,
            follow_redirects: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct InjectionResult {
    pub payload: String,
    pub status_code: u16,
    pub response_length: usize,
    pub response_time_ms: u64,
    pub response_body: String,
    pub analysis: ResponseAnalysis,
}

pub struct InjectionEngine {
    client: Client,
    config: InjectionConfig,
}

impl InjectionEngine {
    pub fn new(config: InjectionConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .redirect(if config.follow_redirects {
                reqwest::redirect::Policy::limited(5)
            } else {
                reqwest::redirect::Policy::none()
            })
            .danger_accept_invalid_certs(true)
            .build()?;
        
        Ok(Self { client, config })
    }
    
    /// Execute single payload injection
    pub async fn inject_single(
        &self,
        request: &HttpRequest,
        point: &InjectionPoint,
        payload: &str,
    ) -> Result<InjectionResult> {
        if self.config.dry_run {
            println!("{}", "[DRY RUN] Would inject payload:".bright_yellow());
            let modified = request.inject_payload(point, payload);
            println!("{}", modified.bright_black());
            
            return Ok(InjectionResult {
                payload: payload.to_string(),
                status_code: 0,
                response_length: 0,
                response_time_ms: 0,
                response_body: String::new(),
                analysis: ResponseAnalysis::default(),
            });
        }
        
        let modified_request = request.inject_payload(point, payload);
        
        let start = Instant::now();
        let response = self.send_request(&modified_request).await?;
        let elapsed = start.elapsed().as_millis() as u64;
        
        let status = response.status().as_u16();
        let body = response.text().await?;
        let length = body.len();
        
        let analysis = ResponseAnalysis::analyze(&body, status, elapsed);
        
        Ok(InjectionResult {
            payload: payload.to_string(),
            status_code: status,
            response_length: length,
            response_time_ms: elapsed,
            response_body: body,
            analysis,
        })
    }
    
    /// Execute multiple payload injections
    pub async fn inject_batch(
        &self,
        request: &HttpRequest,
        point: &InjectionPoint,
        payloads: &[String],
    ) -> Result<Vec<InjectionResult>> {
        let mut results = Vec::new();
        
        for (i, payload) in payloads.iter().enumerate() {
            if self.config.confirm_each {
                println!("\n{} [{}/{}] {}", 
                    "[*]".bright_cyan(), 
                    i + 1, 
                    payloads.len(),
                    payload.bright_white()
                );
                print!("{}   Execute? [Y/n]: ", "›".bright_black());
                std::io::Write::flush(&mut std::io::stdout())?;
                
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                
                if input.trim().to_lowercase() == "n" {
                    println!("{}   Skipped", "›".bright_black());
                    continue;
                }
            }
            
            match self.inject_single(request, point, payload).await {
                Ok(result) => {
                    self.print_result(&result, i + 1, payloads.len());
                    results.push(result);
                }
                Err(e) => {
                    println!("{}   {} {}", "›".bright_black(), "Error:".red(), e);
                }
            }
            
            // Small delay between requests
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        Ok(results)
    }
    
    async fn send_request(&self, raw_request: &str) -> Result<reqwest::Response> {
        // Parse the request to extract components
        let lines: Vec<&str> = raw_request.lines().collect();
        let request_line = lines[0];
        let parts: Vec<&str> = request_line.split_whitespace().collect();
        
        let method = parts[0];
        let path = parts[1];
        
        // Extract host and build URL
        let mut host = String::new();
        let mut headers_map = Vec::new();
        let mut body = String::new();
        let mut in_body = false;
        
        for line in &lines[1..] {
            if in_body {
                body.push_str(line);
                body.push('\n');
            } else if line.is_empty() {
                in_body = true;
            } else if let Some(idx) = line.find(':') {
                let key = line[..idx].trim();
                let value = line[idx + 1..].trim();
                
                if key.to_lowercase() == "host" {
                    host = value.to_string();
                } else {
                    headers_map.push((key, value));
                }
            }
        }
        
        if host.is_empty() {
            anyhow::bail!("No Host header found in request");
        }
        
        let url = format!("http://{}{}", host, path);
        
        // Build request
        let mut req = match method {
            "GET" => self.client.get(&url),
            "POST" => self.client.post(&url),
            "PUT" => self.client.put(&url),
            "DELETE" => self.client.delete(&url),
            "PATCH" => self.client.patch(&url),
            "HEAD" => self.client.head(&url),
            "OPTIONS" => self.client.request(reqwest::Method::OPTIONS, &url),
            _ => self.client.get(&url),
        };
        
        for (key, value) in headers_map {
            req = req.header(key, value);
        }
        
        if !body.trim().is_empty() {
            req = req.body(body.trim().to_string());
        }
        
        Ok(req.send().await?)
    }
    
    fn print_result(&self, result: &InjectionResult, current: usize, total: usize) {
        if self.config.dry_run {
            return;
        }
        
        let status_color = match result.status_code {
            200..=299 => result.status_code.to_string().green(),
            300..=399 => result.status_code.to_string().cyan(),
            400..=499 => result.status_code.to_string().yellow(),
            500..=599 => result.status_code.to_string().red(),
            _ => result.status_code.to_string().white(),
        };
        
        println!("{} [{}/{}] {} | {} | {}ms | {} bytes", 
            "[+]".green(),
            current,
            total,
            status_color,
            result.payload.bright_white(),
            result.response_time_ms,
            result.response_length
        );
        
        // Print significant findings
        if result.analysis.has_error_indicators {
            println!("{}   {} Potential error indicators detected", "›".bright_black(), "!".yellow());
        }
        
        if result.analysis.is_slow_response {
            println!("{}   {} Slow response ({}ms threshold)", "›".bright_black(), "!".yellow(), result.analysis.slow_threshold_ms);
        }
        
        if !result.analysis.interesting_strings.is_empty() {
            println!("{}   {} Interesting: {}", 
                "›".bright_black(), 
                "!".yellow(), 
                result.analysis.interesting_strings.join(", ").cyan()
            );
        }
    }
}
