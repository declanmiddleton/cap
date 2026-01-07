use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub cookies: HashMap<String, String>,
    pub query_params: HashMap<String, String>,
    pub body_params: HashMap<String, String>,
    pub body: String,
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InjectionPoint {
    QueryParam(String),
    BodyParam(String),
    Header(String),
    Cookie(String),
}

impl InjectionPoint {
    pub fn display(&self) -> String {
        match self {
            InjectionPoint::QueryParam(name) => format!("Query: {}", name),
            InjectionPoint::BodyParam(name) => format!("Body: {}", name),
            InjectionPoint::Header(name) => format!("Header: {}", name),
            InjectionPoint::Cookie(name) => format!("Cookie: {}", name),
        }
    }
}

impl HttpRequest {
    /// Load and parse HTTP request from file
    pub async fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = tokio::fs::read_to_string(path.as_ref())
            .await
            .context("Failed to read request file")?;
        
        Self::parse(&content)
    }
    
    /// Parse raw HTTP request
    pub fn parse(raw: &str) -> Result<Self> {
        let lines: Vec<&str> = raw.lines().collect();
        
        if lines.is_empty() {
            anyhow::bail!("Empty request");
        }
        
        // Parse request line
        let request_line = lines[0];
        let parts: Vec<&str> = request_line.split_whitespace().collect();
        if parts.len() < 2 {
            anyhow::bail!("Invalid request line");
        }
        
        let method = parts[0].to_string();
        let full_path = parts[1];
        let version = parts.get(2).unwrap_or(&"HTTP/1.1").to_string();
        
        // Split path and query string
        let (path, query_string) = if let Some(idx) = full_path.find('?') {
            (full_path[..idx].to_string(), &full_path[idx + 1..])
        } else {
            (full_path.to_string(), "")
        };
        
        // Parse query parameters
        let query_params = Self::parse_params(query_string);
        
        // Parse headers
        let mut headers = HashMap::new();
        let mut cookies = HashMap::new();
        let mut i = 1;
        
        while i < lines.len() && !lines[i].is_empty() {
            if let Some(idx) = lines[i].find(':') {
                let key = lines[i][..idx].trim().to_string();
                let value = lines[i][idx + 1..].trim().to_string();
                
                if key.to_lowercase() == "cookie" {
                    cookies = Self::parse_cookies(&value);
                } else {
                    headers.insert(key, value);
                }
            }
            i += 1;
        }
        
        // Parse body
        let body = if i < lines.len() {
            lines[i + 1..].join("\n")
        } else {
            String::new()
        };
        
        // Parse body parameters (if Content-Type is form data)
        let body_params = if headers.get("Content-Type")
            .map(|ct| ct.contains("application/x-www-form-urlencoded"))
            .unwrap_or(false)
        {
            Self::parse_params(&body)
        } else {
            HashMap::new()
        };
        
        Ok(HttpRequest {
            method,
            path,
            version,
            headers,
            cookies,
            query_params,
            body_params,
            body,
            raw: raw.to_string(),
        })
    }
    
    /// Extract all available injection points
    pub fn injection_points(&self) -> Vec<InjectionPoint> {
        let mut points = Vec::new();
        
        for key in self.query_params.keys() {
            points.push(InjectionPoint::QueryParam(key.clone()));
        }
        
        for key in self.body_params.keys() {
            points.push(InjectionPoint::BodyParam(key.clone()));
        }
        
        for key in self.headers.keys() {
            points.push(InjectionPoint::Header(key.clone()));
        }
        
        for key in self.cookies.keys() {
            points.push(InjectionPoint::Cookie(key.clone()));
        }
        
        points
    }
    
    /// Build modified request with payload injected at specified point
    pub fn inject_payload(&self, point: &InjectionPoint, payload: &str) -> String {
        let mut req = self.clone();
        
        match point {
            InjectionPoint::QueryParam(name) => {
                req.query_params.insert(name.clone(), payload.to_string());
                req.rebuild_with_query()
            }
            InjectionPoint::BodyParam(name) => {
                req.body_params.insert(name.clone(), payload.to_string());
                req.rebuild_with_body()
            }
            InjectionPoint::Header(name) => {
                req.headers.insert(name.clone(), payload.to_string());
                req.rebuild_with_headers()
            }
            InjectionPoint::Cookie(name) => {
                req.cookies.insert(name.clone(), payload.to_string());
                req.rebuild_with_cookies()
            }
        }
    }
    
    fn rebuild_with_query(&self) -> String {
        let query = self.build_query_string();
        let path_with_query = if query.is_empty() {
            self.path.clone()
        } else {
            format!("{}?{}", self.path, query)
        };
        
        let mut lines = vec![format!("{} {} {}", self.method, path_with_query, self.version)];
        
        for (k, v) in &self.headers {
            lines.push(format!("{}: {}", k, v));
        }
        
        if !self.cookies.is_empty() {
            lines.push(format!("Cookie: {}", self.build_cookie_string()));
        }
        
        lines.push(String::new());
        
        if !self.body.is_empty() {
            lines.push(self.body.clone());
        }
        
        lines.join("\r\n")
    }
    
    fn rebuild_with_body(&self) -> String {
        let body_string = self.build_body_string();
        let mut req = self.clone();
        req.body = body_string.clone();
        req.headers.insert("Content-Length".to_string(), body_string.len().to_string());
        req.rebuild_with_query()
    }
    
    fn rebuild_with_headers(&self) -> String {
        self.rebuild_with_query()
    }
    
    fn rebuild_with_cookies(&self) -> String {
        self.rebuild_with_query()
    }
    
    fn build_query_string(&self) -> String {
        self.query_params
            .iter()
            .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&")
    }
    
    fn build_body_string(&self) -> String {
        self.body_params
            .iter()
            .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&")
    }
    
    fn build_cookie_string(&self) -> String {
        self.cookies
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("; ")
    }
    
    fn parse_params(params: &str) -> HashMap<String, String> {
        let mut map = HashMap::new();
        
        for pair in params.split('&') {
            if let Some(idx) = pair.find('=') {
                let key = urlencoding::decode(&pair[..idx]).unwrap_or_default().to_string();
                let value = urlencoding::decode(&pair[idx + 1..]).unwrap_or_default().to_string();
                map.insert(key, value);
            }
        }
        
        map
    }
    
    fn parse_cookies(cookie_str: &str) -> HashMap<String, String> {
        let mut map = HashMap::new();
        
        for pair in cookie_str.split(';') {
            let pair = pair.trim();
            if let Some(idx) = pair.find('=') {
                let key = pair[..idx].trim().to_string();
                let value = pair[idx + 1..].trim().to_string();
                map.insert(key, value);
            }
        }
        
        map
    }
    
    /// Get target URL (requires Host header)
    pub fn get_url(&self) -> Option<String> {
        let host = self.headers.get("Host")?;
        let scheme = if self.headers.get("X-Forwarded-Proto")
            .map(|p| p == "https")
            .unwrap_or(false)
        {
            "https"
        } else {
            "http"
        };
        
        Some(format!("{}://{}{}", scheme, host, self.path))
    }
}
