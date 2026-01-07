use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use colored::Colorize;
use reqwest::Client;
use std::collections::HashMap;
use std::time::Duration;

use crate::web::{WebModule, ModuleInfo, ModuleContext, ExecutionResult};

pub struct TechFingerprintModule;

impl TechFingerprintModule {
    pub fn new() -> Self {
        Self
    }
    
    async fn fingerprint(&self, url: &str) -> Result<HashMap<String, Vec<String>>> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::limited(3))
            .danger_accept_invalid_certs(true)
            .build()?;
        
        let mut findings = HashMap::new();
        
        // Make initial request
        let response = client.get(url).send().await?;
        let status = response.status();
        let headers = response.headers().clone();
        let body = response.text().await?;
        
        // Analyze headers
        let mut header_findings = Vec::new();
        
        if let Some(server) = headers.get("server") {
            header_findings.push(format!("Server: {}", server.to_str().unwrap_or("")));
        }
        
        if let Some(powered_by) = headers.get("x-powered-by") {
            header_findings.push(format!("X-Powered-By: {}", powered_by.to_str().unwrap_or("")));
        }
        
        if let Some(aspnet) = headers.get("x-aspnet-version") {
            header_findings.push(format!("ASP.NET Version: {}", aspnet.to_str().unwrap_or("")));
        }
        
        if let Some(aspnetmvc) = headers.get("x-aspnetmvc-version") {
            header_findings.push(format!("ASP.NET MVC Version: {}", aspnetmvc.to_str().unwrap_or("")));
        }
        
        // Security headers
        let mut security_findings = Vec::new();
        
        if headers.get("x-frame-options").is_none() {
            security_findings.push("X-Frame-Options missing (clickjacking risk)".to_string());
        }
        
        if headers.get("x-content-type-options").is_none() {
            security_findings.push("X-Content-Type-Options missing (MIME sniffing risk)".to_string());
        }
        
        if headers.get("strict-transport-security").is_none() {
            security_findings.push("Strict-Transport-Security missing (HTTPS downgrade risk)".to_string());
        }
        
        if headers.get("content-security-policy").is_none() {
            security_findings.push("Content-Security-Policy missing (XSS risk)".to_string());
        }
        
        if headers.get("x-xss-protection").is_none() {
            security_findings.push("X-XSS-Protection missing".to_string());
        }
        
        // Detect WAF/CDN
        let mut waf_findings = Vec::new();
        
        for (name, value) in headers.iter() {
            let name_lower = name.as_str().to_lowercase();
            let value_str = value.to_str().unwrap_or("").to_lowercase();
            
            if name_lower.contains("cloudflare") || value_str.contains("cloudflare") {
                waf_findings.push("Cloudflare detected".to_string());
            }
            
            if name_lower.contains("x-cdn") || value_str.contains("akamai") {
                waf_findings.push("CDN detected (likely Akamai)".to_string());
            }
            
            if name_lower.contains("x-sucuri") || value_str.contains("sucuri") {
                waf_findings.push("Sucuri WAF detected".to_string());
            }
            
            if name_lower.contains("x-modsecurity") || value_str.contains("modsecurity") {
                waf_findings.push("ModSecurity detected".to_string());
            }
        }
        
        // Cookie analysis
        let mut cookie_findings = Vec::new();
        
        for cookie in headers.get_all("set-cookie") {
            if let Ok(cookie_str) = cookie.to_str() {
                cookie_findings.push(cookie_str.to_string());
                
                // Check for framework indicators
                if cookie_str.contains("PHPSESSID") {
                    findings.entry("framework".to_string())
                        .or_insert_with(Vec::new)
                        .push("PHP detected (PHPSESSID cookie)".to_string());
                }
                
                if cookie_str.contains("JSESSIONID") {
                    findings.entry("framework".to_string())
                        .or_insert_with(Vec::new)
                        .push("Java/JSP detected (JSESSIONID cookie)".to_string());
                }
                
                if cookie_str.contains("ASP.NET_SessionId") {
                    findings.entry("framework".to_string())
                        .or_insert_with(Vec::new)
                        .push("ASP.NET detected (SessionId cookie)".to_string());
                }
                
                // Check for secure flags
                if !cookie_str.contains("Secure") {
                    security_findings.push(format!("Cookie without Secure flag: {}", cookie_str.split(';').next().unwrap_or("")));
                }
                
                if !cookie_str.contains("HttpOnly") {
                    security_findings.push(format!("Cookie without HttpOnly flag: {}", cookie_str.split(';').next().unwrap_or("")));
                }
            }
        }
        
        // Body analysis - framework signatures
        let mut framework_findings = Vec::new();
        let body_lower = body.to_lowercase();
        
        if body_lower.contains("django") || body_lower.contains("csrfmiddlewaretoken") {
            framework_findings.push("Django (Python) detected".to_string());
        }
        
        if body_lower.contains("flask") || body_lower.contains("werkzeug") {
            framework_findings.push("Flask (Python) detected".to_string());
        }
        
        if body_lower.contains("laravel") || body_lower.contains("laravel_session") {
            framework_findings.push("Laravel (PHP) detected".to_string());
        }
        
        if body_lower.contains("symfony") {
            framework_findings.push("Symfony (PHP) detected".to_string());
        }
        
        if body_lower.contains("spring") || body_lower.contains("whitelabel error page") {
            framework_findings.push("Spring Boot (Java) detected".to_string());
        }
        
        if body_lower.contains("express") {
            framework_findings.push("Express.js (Node.js) detected".to_string());
        }
        
        if body_lower.contains("rails") || body_lower.contains("ruby") {
            framework_findings.push("Ruby on Rails detected".to_string());
        }
        
        if body_lower.contains("asp.net") || body_lower.contains("__viewstate") {
            framework_findings.push("ASP.NET detected".to_string());
        }
        
        // JavaScript frameworks
        if body.contains("ng-app") || body.contains("angular") {
            framework_findings.push("Angular (frontend) detected".to_string());
        }
        
        if body.contains("react") || body.contains("reactjs") {
            framework_findings.push("React (frontend) detected".to_string());
        }
        
        if body.contains("vue") || body.contains("v-bind") {
            framework_findings.push("Vue.js (frontend) detected".to_string());
        }
        
        // CMS detection
        if body_lower.contains("wordpress") || body_lower.contains("wp-content") {
            framework_findings.push("WordPress CMS detected".to_string());
        }
        
        if body_lower.contains("joomla") {
            framework_findings.push("Joomla CMS detected".to_string());
        }
        
        if body_lower.contains("drupal") {
            framework_findings.push("Drupal CMS detected".to_string());
        }
        
        // Test for common paths
        let test_paths = vec![
            "/robots.txt",
            "/.git/config",
            "/.env",
            "/admin",
            "/phpmyadmin",
            "/wp-admin",
            "/api",
        ];
        
        let mut path_findings = Vec::new();
        
        for path in test_paths {
            let test_url = format!("{}{}", url.trim_end_matches('/'), path);
            if let Ok(resp) = client.get(&test_url).send().await {
                if resp.status().is_success() {
                    path_findings.push(format!("{} - Status {}", path, resp.status()));
                }
            }
            
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        
        // Compile all findings
        findings.insert("headers".to_string(), header_findings);
        findings.insert("security".to_string(), security_findings);
        findings.insert("waf_cdn".to_string(), waf_findings);
        findings.insert("cookies".to_string(), cookie_findings);
        findings.insert("framework".to_string(), framework_findings);
        findings.insert("paths".to_string(), path_findings);
        findings.insert("status".to_string(), vec![format!("HTTP Status: {}", status)]);
        
        Ok(findings)
    }
}

#[async_trait]
impl WebModule for TechFingerprintModule {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "web/fingerprint/tech".to_string(),
            name: "Technology Fingerprinting".to_string(),
            category: "Fingerprinting".to_string(),
            description: "Comprehensive technology fingerprinting using safe requests. Gathers HTTP headers, server banners, cookies, security headers, framework indicators, CMS detection, WAF/CDN identification, and common path discovery.".to_string(),
            author: "CAP Framework".to_string(),
            references: vec![
                "https://owasp.org/www-project-web-security-testing-guide/latest/4-Web_Application_Security_Testing/01-Information_Gathering/08-Fingerprint_Web_Application_Framework".to_string(),
            ],
        }
    }
    
    fn required_options(&self) -> Vec<String> {
        vec![
            "URL (target URL)".to_string(),
        ]
    }
    
    async fn execute(&self, context: &ModuleContext) -> Result<ExecutionResult> {
        let url = context.get_option("URL")
            .ok_or_else(|| anyhow::anyhow!("No URL specified. Use 'set URL <target>'"))?;
        
        println!("\n{} Fingerprinting target: {}", "[*]".bright_cyan(), url.bright_white());
        println!();
        
        let findings_map = self.fingerprint(url).await?;
        
        let mut findings = Vec::new();
        
        // Display and collect findings
        for (category, items) in &findings_map {
            if !items.is_empty() {
                println!("{}", format!("{}:", category.to_uppercase()).bright_yellow());
                for item in items {
                    println!("{}   {}", "›".bright_black(), item);
                    findings.push(format!("[{}] {}", category, item));
                }
                println!();
            }
        }
        
        if findings.is_empty() {
            findings.push("No significant findings from fingerprinting".to_string());
        }
        
        Ok(ExecutionResult {
            success: true,
            findings,
            injection_results: Vec::new(),
            module_id: self.info().id,
            timestamp: Utc::now(),
        })
    }
}
