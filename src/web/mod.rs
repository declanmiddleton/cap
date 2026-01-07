pub mod request;
pub mod injection;
pub mod analyzer;
pub mod modules;

use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use colored::Colorize;
use std::collections::HashMap;

use request::{HttpRequest, InjectionPoint};
use injection::{InjectionConfig, InjectionEngine, InjectionResult};

#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub id: String,
    pub name: String,
    pub category: String,
    pub description: String,
    pub author: String,
    pub references: Vec<String>,
    pub examples: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ModuleContext {
    pub request: Option<HttpRequest>,
    pub injection_point: Option<InjectionPoint>,
    pub config: HashMap<String, String>,
    pub operator: String,
}

impl ModuleContext {
    pub fn new(operator: String) -> Self {
        Self {
            request: None,
            injection_point: None,
            config: HashMap::new(),
            operator,
        }
    }
    
    pub fn set_option(&mut self, key: String, value: String) {
        self.config.insert(key, value);
    }
    
    pub fn get_option(&self, key: &str) -> Option<&String> {
        self.config.get(key)
    }
}

#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub success: bool,
    pub findings: Vec<String>,
    pub injection_results: Vec<InjectionResult>,
    pub module_id: String,
    pub timestamp: chrono::DateTime<Utc>,
}

#[async_trait]
pub trait WebModule: Send + Sync {
    fn info(&self) -> ModuleInfo;
    fn required_options(&self) -> Vec<String>;
    async fn execute(&self, context: &ModuleContext) -> Result<ExecutionResult>;
    
    fn display_info(&self) {
        let info = self.info();
        println!();
        println!("{}", format!("Module: {}", info.name).bright_cyan().bold());
        println!("{}", format!("ID: {}", info.id).bright_black());
        println!();
        println!("{}:", "Description".bright_yellow());
        println!("  {}", info.description);
        println!();
        println!("{}: {}", "Category".bright_yellow(), info.category);
        println!("{}: {}", "Author".bright_yellow(), info.author.bright_black());
        
        println!();
        println!("{}:", "Required Options".bright_yellow());
        for opt in self.required_options() {
            println!("  {} {}", "›".bright_black(), opt);
        }
        
        if !info.examples.is_empty() {
            println!();
            println!("{}:", "Example Usage".bright_green().bold());
            for (i, example) in info.examples.iter().enumerate() {
                if info.examples.len() > 1 {
                    println!("\n  {}. {}", i + 1, "Example:".bright_white());
                }
                println!("  {}", "$".bright_black());
                println!("  {}", example.cyan());
            }
        }
        
        if !info.references.is_empty() {
            println!();
            println!("{}:", "References".bright_yellow());
            for ref_link in &info.references {
                println!("  {} {}", "›".bright_black(), ref_link.bright_blue());
            }
        }
        
        println!();
    }
}

pub struct ModuleRegistry {
    modules: HashMap<String, Box<dyn WebModule>>,
}

impl ModuleRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            modules: HashMap::new(),
        };
        
        // Register all modules
        registry.register_ssti_modules();
        registry.register_sqli_modules();
        registry.register_fingerprint_modules();
        
        registry
    }
    
    fn register(&mut self, module: Box<dyn WebModule>) {
        let id = module.info().id.clone();
        self.modules.insert(id, module);
    }
    
    pub fn list_modules(&self) -> Vec<ModuleInfo> {
        self.modules
            .values()
            .map(|m| m.info())
            .collect()
    }
    
    pub fn get_module(&self, id: &str) -> Option<&Box<dyn WebModule>> {
        self.modules.get(id)
    }
    
    pub fn list_by_category(&self, category: &str) -> Vec<ModuleInfo> {
        self.modules
            .values()
            .filter(|m| m.info().category == category)
            .map(|m| m.info())
            .collect()
    }
    
    pub fn display_modules(&self) {
        let mut by_category: HashMap<String, Vec<ModuleInfo>> = HashMap::new();
        
        for module in self.list_modules() {
            by_category
                .entry(module.category.clone())
                .or_insert_with(Vec::new)
                .push(module);
        }
        
        println!();
        println!("{}", "Web Application Modules".bright_cyan().bold());
        println!();
        
        for (category, modules) in by_category.iter() {
            println!("{}", format!("{}:", category).bright_yellow());
            
            for module in modules {
                println!("  {} {} - {}", 
                    "›".bright_black(),
                    module.id.bright_white(),
                    module.name.bright_black()
                );
            }
            println!();
        }
    }
    
    fn register_ssti_modules(&mut self) {
        self.register(Box::new(modules::ssti::Jinja2Module::new()));
        self.register(Box::new(modules::ssti::FreemarkerModule::new()));
        self.register(Box::new(modules::ssti::TwigModule::new()));
        self.register(Box::new(modules::ssti::VelocityModule::new()));
        self.register(Box::new(modules::ssti::SSTIDetectorModule::new()));
    }
    
    fn register_sqli_modules(&mut self) {
        self.register(Box::new(modules::sqli::BooleanSQLiModule::new()));
        self.register(Box::new(modules::sqli::ErrorSQLiModule::new()));
        self.register(Box::new(modules::sqli::TimeSQLiModule::new()));
    }
    
    fn register_fingerprint_modules(&mut self) {
        self.register(Box::new(modules::fingerprint::TechFingerprintModule::new()));
    }
}

impl Default for ModuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}
