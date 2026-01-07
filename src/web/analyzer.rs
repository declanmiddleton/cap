use regex::Regex;

#[derive(Debug, Clone, Default)]
pub struct ResponseAnalysis {
    pub has_error_indicators: bool,
    pub is_slow_response: bool,
    pub slow_threshold_ms: u64,
    pub interesting_strings: Vec<String>,
    pub detected_technologies: Vec<String>,
    pub security_headers_missing: Vec<String>,
}

impl ResponseAnalysis {
    pub fn analyze(body: &str, status_code: u16, response_time_ms: u64) -> Self {
        let mut analysis = ResponseAnalysis {
            slow_threshold_ms: 5000,
            ..Default::default()
        };
        
        // Check for error indicators
        analysis.has_error_indicators = Self::detect_errors(body, status_code);
        
        // Check response time
        analysis.is_slow_response = response_time_ms > analysis.slow_threshold_ms;
        
        // Extract interesting strings
        analysis.interesting_strings = Self::find_interesting_strings(body);
        
        // Detect technologies
        analysis.detected_technologies = Self::detect_technologies(body);
        
        analysis
    }
    
    fn detect_errors(body: &str, status_code: u16) -> bool {
        if status_code >= 500 {
            return true;
        }
        
        let error_patterns = [
            "sql syntax",
            "mysql error",
            "postgresql error",
            "oracle error",
            "sqlite error",
            "syntax error",
            "unclosed quotation",
            "unexpected token",
            "traceback",
            "stack trace",
            "exception",
            "fatal error",
            "warning:",
            "notice:",
            "parse error",
            "undefined variable",
            "undefined index",
            "database error",
            "query failed",
        ];
        
        let body_lower = body.to_lowercase();
        
        for pattern in &error_patterns {
            if body_lower.contains(pattern) {
                return true;
            }
        }
        
        false
    }
    
    fn find_interesting_strings(body: &str) -> Vec<String> {
        let mut findings = Vec::new();
        
        // Template engine outputs
        if body.contains("{{") || body.contains("{%") {
            findings.push("Template syntax detected".to_string());
        }
        
        // SQL results
        if let Ok(re) = Regex::new(r"(SELECT|INSERT|UPDATE|DELETE)\s+.*\s+FROM") {
            if re.is_match(body) {
                findings.push("SQL query visible".to_string());
            }
        }
        
        // Admin/debug info
        if body.contains("DEBUG") || body.contains("DEVELOPMENT") {
            findings.push("Debug mode indicated".to_string());
        }
        
        // Version disclosure
        if let Ok(re) = Regex::new(r"version\s+[\d.]+") {
            if re.is_match(body) {
                findings.push("Version disclosure".to_string());
            }
        }
        
        // File paths
        if let Ok(re) = Regex::new(r"(/[a-zA-Z0-9_./]+){3,}|([A-Z]:\\[a-zA-Z0-9_\\]+){2,}") {
            if re.is_match(body) {
                findings.push("File path disclosure".to_string());
            }
        }
        
        findings
    }
    
    fn detect_technologies(body: &str) -> Vec<String> {
        let mut techs = Vec::new();
        
        // Framework signatures
        let signatures = [
            ("Django", vec!["csrfmiddlewaretoken", "django", "__admin__"]),
            ("Flask", vec!["flask", "werkzeug"]),
            ("Express", vec!["express", "x-powered-by: express"]),
            ("Laravel", vec!["laravel", "laravel_session"]),
            ("Spring", vec!["spring", "jsessionid", "whitelabel error page"]),
            ("ASP.NET", vec!["asp.net", "viewstate", "__viewstate"]),
            ("PHP", vec!["phpsessid", ".php"]),
            ("Ruby on Rails", vec!["rails", "ruby", "_session_id"]),
        ];
        
        let body_lower = body.to_lowercase();
        
        for (name, patterns) in &signatures {
            for pattern in patterns {
                if body_lower.contains(pattern) {
                    techs.push(name.to_string());
                    break;
                }
            }
        }
        
        techs
    }
    
    pub fn has_significant_findings(&self) -> bool {
        self.has_error_indicators 
            || self.is_slow_response 
            || !self.interesting_strings.is_empty()
    }
}
