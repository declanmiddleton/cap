use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;

use crate::web::{WebModule, ModuleInfo, ModuleContext, ExecutionResult};
use crate::web::injection::{InjectionConfig, InjectionEngine};

pub struct SSTIDetectorModule;

impl SSTIDetectorModule {
    pub fn new() -> Self {
        Self
    }
    
    fn get_detection_payloads(&self) -> Vec<(String, Vec<&'static str>)> {
        vec![
            // (payload, expected_engines)
            ("{{7*7}}".to_string(), vec!["Jinja2", "Twig"]),
            ("${7*7}".to_string(), vec!["Freemarker", "Velocity", "Thymeleaf"]),
            ("#set($x=7*7)$x".to_string(), vec!["Velocity"]),
            ("${7*'7'}".to_string(), vec!["Freemarker"]),
            ("{{7*'7'}}".to_string(), vec!["Jinja2"]),
            ("<#assign ex=\"freemarker.template.utility.Execute\"?new()>".to_string(), vec!["Freemarker"]),
            ("{{\"Hello\"~\"World\"}}".to_string(), vec!["Twig"]),
            ("@(7*7)".to_string(), vec!["Razor"]),
            ("#{7*7}".to_string(), vec!["JSF", "Expression Language"]),
            ("*{7*7}".to_string(), vec!["Thymeleaf"]),
            ("%{7*7}".to_string(), vec!["OGNL", "Struts"]),
            ("${T(java.lang.Runtime).getRuntime()}".to_string(), vec!["Spring EL"]),
        ]
    }
}

#[async_trait]
impl WebModule for SSTIDetectorModule {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "web/ssti/detector".to_string(),
            name: "SSTI Detection & Engine Identification".to_string(),
            category: "SSTI".to_string(),
            description: "Low-impact detection probes to identify template engine without exploitation. Tests multiple template syntaxes to fingerprint the backend engine.".to_string(),
            author: "CAP Framework".to_string(),
            references: vec![
                "https://book.hacktricks.xyz/pentesting-web/ssti-server-side-template-injection".to_string(),
                "https://github.com/swisskyrepo/PayloadsAllTheThings/tree/master/Server%20Side%20Template%20Injection".to_string(),
            ],
            examples: vec![
                "cap web run --module web/ssti/detector --request ./requests/search.req --injection-point query".to_string(),
                "cap web run --module web/ssti/detector --request ./requests/search.req --injection-point query --dry-run".to_string(),
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
        
        let detection_payloads = self.get_detection_payloads();
        let payloads: Vec<String> = detection_payloads.iter()
            .map(|(p, _)| p.clone())
            .collect();
        
        let config = InjectionConfig {
            dry_run: context.get_option("DRY_RUN").map(|s| s == "true").unwrap_or(false),
            confirm_each: false, // Detection should be automated
            timeout_secs: 10,
            follow_redirects: false,
        };
        
        let engine = InjectionEngine::new(config)?;
        let results = engine.inject_batch(request, injection_point, &payloads).await?;
        
        let mut findings = Vec::new();
        let mut engine_scores: HashMap<&str, u32> = HashMap::new();
        
        // Analyze results
        for (i, result) in results.iter().enumerate() {
            let (payload, engines) = &detection_payloads[i];
            
            // Check for mathematical evaluation
            if result.response_body.contains("49") {
                findings.push(format!("Template evaluation detected with: {}", payload));
                
                for engine in engines {
                    *engine_scores.entry(engine).or_insert(0) += 10;
                }
            }
            
            // Check for expected specific outputs
            if payload.contains("7*'7'") && result.response_body.contains("7777777") {
                findings.push(format!("String multiplication detected (Jinja2 signature): {}", payload));
                *engine_scores.entry("Jinja2").or_insert(0) += 20;
            }
            
            if payload.contains("Hello") && result.response_body.contains("HelloWorld") {
                findings.push(format!("String concatenation detected (Twig signature): {}", payload));
                *engine_scores.entry("Twig").or_insert(0) += 20;
            }
            
            // Check for error-based fingerprinting
            if result.analysis.has_error_indicators {
                let body_lower = result.response_body.to_lowercase();
                
                if body_lower.contains("jinja") {
                    findings.push("Jinja2 error signature detected".to_string());
                    *engine_scores.entry("Jinja2").or_insert(0) += 15;
                }
                
                if body_lower.contains("freemarker") {
                    findings.push("Freemarker error signature detected".to_string());
                    *engine_scores.entry("Freemarker").or_insert(0) += 15;
                }
                
                if body_lower.contains("twig") {
                    findings.push("Twig error signature detected".to_string());
                    *engine_scores.entry("Twig").or_insert(0) += 15;
                }
                
                if body_lower.contains("velocity") {
                    findings.push("Velocity error signature detected".to_string());
                    *engine_scores.entry("Velocity").or_insert(0) += 15;
                }
            }
        }
        
        // Determine most likely template engine
        if let Some((engine, score)) = engine_scores.iter().max_by_key(|(_, s)| *s) {
            if *score > 10 {
                findings.push(format!("Most likely template engine: {} (confidence score: {})", engine, score));
                findings.push(format!("Recommended module: web/ssti/{}", engine.to_lowercase()));
            }
        }
        
        if findings.is_empty() {
            findings.push("No SSTI detected with tested payloads".to_string());
        }
        
        Ok(ExecutionResult {
            success: !engine_scores.is_empty(),
            findings,
            injection_results: results,
            module_id: self.info().id,
            timestamp: Utc::now(),
        })
    }
}
