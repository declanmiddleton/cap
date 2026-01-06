# CAP - Comprehensive Assessment Platform

<div align="center">

```
    ██████╗ █████╗ ████████╗
   ██╔════╝██╔══██╗██╔═══██║██║     
   ██║     ███████║██████╔╝
   ██║     ██╔══██║██╔═══╝ 
   ╚██████╗██║  ██║██║     
    ╚═════╝╚═╝  ╚═╝╚═╝     
```

**A modern, Rust-based security orchestration framework for authorized penetration testing and defensive research**

[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

⚠️ **AUTHORIZED USE ONLY** - For security research, training, and approved testing ⚠️

</div>

---

## 🚀 Quick Start

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build CAP
cd custom-c2
cargo build --release

# Initialize project
./target/release/cap init my-assessment
cd my-assessment

# Add authorized targets
../target/release/cap scope add example.com

# View available modules
../target/release/cap modules

# Run web enumeration
../target/release/cap module --name web-enum --target https://example.com
```

---

## 📖 What is CAP?

CAP (Comprehensive Assessment Platform) is a research-oriented security orchestration framework built from the ground up in Rust. Unlike traditional C2 frameworks, CAP prioritizes **security, transparency, and accountability** with innovative features designed for defensive security research and authorized penetration testing.

### Key Differentiators

- **🔒 Mandatory Scope Enforcement** - All operations require explicit target authorization
- **🔐 Cryptographic Audit Chain** - SHA-256 hash-chained immutable audit logs
- **⏰ Time-Bounded Sessions** - Auto-expiring sessions (24h default) enforce re-authorization
- **🎯 Safe Defaults** - Localhost-only binding, read-only operations, no exploitation
- **⚡ Modern Architecture** - Built with Rust for memory safety and performance
- **🌐 Dual Interface** - Both CLI and REST API for flexibility

---

## ✨ Features

### Core Framework

| Feature | Description |
|---------|-------------|
| **Scope Management** | Whitelist-based targeting (IP/CIDR, domains, wildcards) |
| **Session Management** | Time-bounded access with automatic expiration |
| **Audit Logging** | Tamper-evident logs with cryptographic integrity |
| **Configuration** | TOML-based with environment-specific configs |
| **API Server** | RESTful API for automation and integration |

### Security Modules

| Module | Purpose | Features |
|--------|---------|----------|
| **web-enum** | Web application enumeration | Wordlist-based discovery, status code filtering, verbose mode |
| **dns-enum** | DNS & subdomain enumeration | Fast concurrent resolution, IPv4/IPv6 support |
| **port-scan** | Network port scanning | Common ports, service detection, concurrent scanning |

### Innovative Security Features

- **Cryptographic Audit Trail**: Each log entry contains SHA-256 hash of previous entry
- **Scope Enforcement**: Blocks all operations against unauthorized targets
- **Operator Attribution**: Every action tied to system username
- **Integrity Verification**: Built-in tamper detection for audit logs
- **Persistent Configuration**: Scope changes saved immediately

---

## 📸 Screenshots

### Main Interface
<!-- Add screenshot here -->
![CAP Main Interface](screenshots/main-interface.png)

### Module List
<!-- Add screenshot here -->
![Available Modules](screenshots/modules-list.png)

### Web Enumeration in Action
<!-- Add screenshot here -->
![Web Enumeration](screenshots/web-enum-scan.png)

### Scope Management
<!-- Add screenshot here -->
![Scope Management](screenshots/scope-management.png)

### Audit Logs
<!-- Add screenshot here -->
![Audit Logs](screenshots/audit-logs.png)

---

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────┐
│                      CAP Framework                       │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ┌────────────┐  ┌──────────┐  ┌─────────────────┐    │
│  │    CLI     │  │   API    │  │  Session Mgmt   │    │
│  │ Interface  │  │ (Axum)   │  │   (Time-bound)  │    │
│  └────────────┘  └──────────┘  └─────────────────┘    │
│                                                          │
│  ┌────────────────────────────────────────────────┐    │
│  │          Core Framework                         │    │
│  │  • Scope Enforcement (IP/Domain whitelist)     │    │
│  │  • Audit Logger (Cryptographic hash chain)     │    │
│  │  • Configuration Management                     │    │
│  └────────────────────────────────────────────────┘    │
│                                                          │
│  ┌────────────────────────────────────────────────┐    │
│  │          Security Modules (Pluggable)          │    │
│  │  • Web Enumeration                             │    │
│  │  • DNS/Subdomain Discovery                     │    │
│  │  • Port Scanning                               │    │
│  └────────────────────────────────────────────────┘    │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

---

## 🔧 Installation

### Prerequisites

- Rust 1.70+ ([Install Rust](https://rustup.rs/))
- Linux, macOS, or Windows

### Build from Source

```bash
# Clone repository
git clone https://github.com/yourusername/cap.git
cd cap

# Build release binary
cargo build --release

# Binary location: target/release/cap
```

### Optional: Install Globally

```bash
# Create symlink
mkdir -p ~/.local/bin
ln -sf $(pwd)/target/release/cap ~/.local/bin/cap

# Add to PATH (if not already)
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
```

---

## 📚 Usage

### Scope Management

All operations require targets to be explicitly authorized:

```bash
# Add targets to scope
cap scope add example.com
cap scope add 192.168.1.0/24
cap scope add "*.test.com"

# List authorized targets
cap scope list

# Check if target is in scope
cap scope check example.com

# Remove from scope
cap scope remove example.com
```

### Session Management

Sessions automatically expire after 24 hours:

```bash
# Create session
cap session new "Q1 Assessment"

# List active sessions
cap session list

# Terminate session
cap session kill <session-id>
```

### Module Execution

#### Web Enumeration

```bash
# Basic scan
cap module --name web-enum --target https://example.com

# With custom wordlist
cap module --name web-enum --target https://example.com \
  --wordlist /path/to/wordlist.txt \
  --threads 20

# Verbose mode (see all attempts)
cap module --name web-enum --target https://example.com \
  --verbose \
  --threads 10

# Filter status codes
cap module --name web-enum --target https://example.com \
  --status-codes 200,301,403 \
  --threads 20

# Exclude status codes
cap module --name web-enum --target https://example.com \
  --exclude-codes 404,503 \
  --threads 20
```

#### DNS Enumeration

```bash
# Subdomain discovery
cap module --name dns-enum --target example.com

# With custom wordlist
cap module --name dns-enum --target example.com \
  --wordlist /path/to/subdomains.txt \
  --threads 50
```

#### Port Scanning

```bash
# Scan common ports
cap module --name port-scan --target 192.168.1.100

# Fast scan (more threads)
cap module --name port-scan --target 192.168.1.100 \
  --threads 100
```

### Wordlist Management

CAP automatically discovers wordlists in standard locations:

```bash
# List available wordlists
cap wordlists

# Search for specific wordlists
cap wordlists --search directory
cap wordlists --search common
```

Searched locations:
- `/usr/share/wordlists/`
- `/usr/share/seclists/`
- `/snap/seclists/current/`
- `wordlists/` (local)

### Payload Generation

Generate reusable task configurations:

```bash
# Generate payload
cap generate --module web-enum --target example.com

# Save to file
cap generate --module dns-enum --target example.com \
  --output task.json
```

### Audit Logs

All operations are logged with cryptographic integrity:

```bash
# View audit logs
cap audit

# Filter by session
cap audit --session-id <session-id>

# Export for compliance
cap audit --export assessment-report.json
```

### API Server

Start REST API for automation:

```bash
# Start listener
cap listen --host 127.0.0.1 --port 8443

# API endpoints available at:
# - GET  /health
# - GET  /api/sessions
# - POST /api/sessions
# - POST /api/modules/execute
# - GET  /api/scope
# - POST /api/scope
# - GET  /api/audit
```

API Example:

```bash
# Execute module via API
curl -X POST http://localhost:8443/api/modules/execute \
  -H "Content-Type: application/json" \
  -d '{
    "module": "web-enum",
    "target": "https://example.com",
    "threads": 10
  }'
```

---

## 📋 Configuration

Edit `config/default.toml`:

```toml
[general]
name = "My Assessment"
description = "Security assessment project"

[server]
host = "127.0.0.1"
port = 8443
tls_enabled = false

[scope]
authorized_targets = [
    "example.com",
    "192.168.1.0/24",
    "*.test.com"
]

[audit]
log_path = "logs/audit.jsonl"
retention_days = 90

[modules]
default_threads = 10
timeout_seconds = 300
```

---

## 🎓 Use Cases

### Authorized Penetration Testing
- External network assessments
- Web application security testing
- Infrastructure reconnaissance
- Compliance testing (PCI-DSS, NIST)

### Security Research
- Vulnerability research in lab environments
- Tool development and validation
- Attack surface mapping
- Defense technique testing

### Training & Education
- Blue team detection training
- Red team reconnaissance practice
- CTF competition infrastructure
- Security workshop environments

### Continuous Security
- Scheduled security scans
- Regression testing
- Configuration validation
- Asset discovery

---

## 🛡️ Security Model

### Defense-in-Depth

1. **Scope Enforcement Layer** - Blocks unauthorized targeting
2. **Audit Layer** - Cryptographic integrity verification
3. **Session Layer** - Time-bounded access control
4. **Application Layer** - Safe defaults, read-only operations

### Compliance Features

- Complete audit trail of all operations
- Operator identity tracking (system username)
- Authorization reference support
- Tamper-evident logging
- Export capability for reporting
- Configurable log retention

---

## 🚧 Roadmap

### Planned Features

- [ ] TLS/mTLS support for API
- [ ] Persistent session storage (SQLite)
- [ ] HTML/PDF report generation
- [ ] Interactive shell sessions (with full audit)
- [ ] Real-time collaboration (WebSocket)
- [ ] SIEM integration (Splunk, ELK)
- [ ] Module plugin system
- [ ] Advanced authentication methods
- [ ] Distributed execution (multi-agent)

---

## 🤝 Contributing

Contributions are welcome! Please ensure all contributions:

1. Maintain the defensive/research focus
2. Include tests and documentation
3. Follow Rust best practices
4. Add audit logging for new features
5. Respect scope enforcement principles

---

## 📜 License

MIT License - See [LICENSE](LICENSE) file for details.

---

## ⚠️ Legal Disclaimer

**IMPORTANT**: This tool is designed for authorized security testing, research, and educational purposes only.

- ✅ Use only on systems you own or have explicit written permission to test
- ✅ Obtain proper authorization before any security assessment
- ✅ Comply with all applicable laws and regulations
- ✅ Maintain audit logs for compliance and evidence

**Unauthorized access to computer systems is illegal.** The authors are not responsible for any misuse or damage caused by this program. Users are solely responsible for ensuring proper authorization.

---

## 🙏 Acknowledgments

Inspired by:
- **Sliver** - Modern C2 framework architecture
- **Metasploit** - Modular assessment framework design
- **Gobuster/Dirsearch** - Web enumeration tools

Built with:
- **Rust** - Systems programming language
- **Tokio** - Async runtime
- **Axum** - Web framework
- **Clap** - CLI parsing
- **SecLists** - Security testing wordlists

---

## 📞 Contact

- **Issues**: [GitHub Issues](https://github.com/yourusername/cap/issues)
- **Discussions**: [GitHub Discussions](https://github.com/yourusername/cap/discussions)

---

<div align="center">

**Built with ❤️ for the security research community**

*Always ensure proper authorization before testing any system*

</div>
