# CAP

**Central Access Point** — A streamlined shell management and post-exploitation tool built for efficiency.

<img width="1056" height="357" alt="CAP Banner" src="https://github.com/user-attachments/assets/d2359001-218c-47f1-aa99-a00432006854" />

---

## What is CAP?

CAP (Central Access Point) is a **practical penetration testing tool** designed to save time during engagements by simplifying common post-exploitation tasks. It's not a framework—it's a focused utility that handles the tedious work so you can focus on the assessment.

### Why CAP?

Stop wasting time on:
- Upgrading basic shells manually
- Managing multiple reverse shell sessions across tabs
- Copy-pasting privilege escalation commands
- Switching between scattered tools for enumeration
- Losing context when you background a session

CAP consolidates these workflows into a single, efficient interface.

---

## Key Features

### 🎯 **Shell Management**
- **Interactive listener** with clean, guided setup
- **Multi-session handling** — run, background, attach, and switch between shells seamlessly
- **Persistent context** — see target info, privilege level, and session age at a glance
- **Session notes** — annotate shells with custom metadata for tracking

### 🔧 **Built-in Capabilities**
- **Privilege escalation helpers** — common techniques ready to deploy
- **Web vulnerability testing** — SQL injection, SSTI detection, and fingerprinting
- **Network enumeration** — port scanning and DNS discovery
- **Audit logging** — all actions logged with timestamps for reporting

### ⚡ **Time-Saving Design**
- **Single binary** — no dependencies, no installation scripts
- **Scope enforcement** — stay within authorized targets
- **Fast workflows** — everything accessible from one interface

---

## Quick Start

### Installation

**Requirements:** Rust toolchain (1.70+)

```bash
# Clone the repository
git clone https://github.com/declanmiddleton/cap.git
cd cap

# Build the release binary
cargo build --release

# Run CAP
./target/release/cap --help
```

### Basic Usage

```bash
# Start a listener (interactive setup)
cap listen

# List active sessions
cap sessions

# Attach to a session
cap attach <session-id>

# Add notes to a session
cap note <session-id> "Domain Admin shell - DC01"

# Kill a session
cap kill <session-id>
```

---

## Platform Support

| Platform | Status | Notes |
|----------|--------|-------|
| **Linux** | ✅ Fully supported | Debian, Ubuntu, Arch, Kali, Parrot |
| **macOS** | ✅ Fully supported | Apple Silicon & Intel |
| **Windows** | ✅ Fully supported | MSVC toolchain required |

### Building for Linux (Static Binary)

For maximum portability, build a static binary using musl:

```bash
# Install musl target
rustup target add x86_64-unknown-linux-musl

# Build static binary
cargo build --release --target x86_64-unknown-linux-musl

# Binary location
./target/x86_64-unknown-linux-musl/release/cap
```

This creates a standalone executable with no runtime dependencies.

---

## Use Cases

CAP is designed for **authorized security assessments** including:

- 🔴 **Penetration Testing** — streamline post-exploitation and enumeration
- 🟣 **Red Team Operations** — manage multiple access points efficiently
- 🔵 **Blue Team Validation** — test detection and response capabilities
- 🧪 **Security Research** — rapid prototyping and testing
- 🎓 **Training & Labs** — educational environments and CTFs

---

## What CAP Is Not

CAP is **not**:
- ❌ A malware framework
- ❌ An evasion toolkit
- ❌ A general-purpose C2 system
- ❌ Designed for unauthorized access

**Use responsibly.** CAP is intended for authorized engagements only. Users are responsible for compliance with applicable laws and regulations.

---

## Design Philosophy

**Simplicity over complexity** — CAP does a few things well instead of trying to be everything.

- **Shell-first** — built around the reality of post-exploitation work
- **Fast workflows** — reduce friction and context-switching
- **Transparent operations** — audit logs and scope controls built-in
- **Self-contained** — single binary, minimal configuration

CAP is written in Rust for memory safety, performance, and reliability. It produces a single static binary with no runtime dependencies.

---

## Project Status

CAP is under **active development** and will remain **open-source**.

### Roadmap
- [ ] Interactive privilege escalation module
- [ ] Extended protocol support (SMB, SSH)
- [ ] Session history and replay
- [ ] Custom payload generation
- [ ] Enhanced web module capabilities

Contributions, bug reports, and feature requests are welcome.

---

## Installation Details

### Linux (Debian/Ubuntu/Kali/Parrot)

```bash
# Install Rust via package manager
sudo apt update
sudo apt install cargo rustc

# Clone and build
git clone https://github.com/declanmiddleton/cap.git
cd cap
cargo build --release
```

### Linux (Arch-based)

```bash
# Install Rust
sudo pacman -S rust cargo

# Clone and build
git clone https://github.com/declanmiddleton/cap.git
cd cap
cargo build --release
```

### macOS

```bash
# Install Rust via Homebrew
brew install rust

# Or use the official installer
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/declanmiddleton/cap.git
cd cap
cargo build --release
```

### Windows

```powershell
# Install Rust from https://rustup.rs (select MSVC toolchain)

# Clone and build
git clone https://github.com/declanmiddleton/cap.git
cd cap
cargo build --release
```

**Note:** Windows Defender may flag security tools. Add exclusions in lab environments as needed.

---

## License

See [LICENSE](LICENSE) for details.

---

## Disclaimer

This tool is provided for **authorized security testing and research purposes only**. Unauthorized access to computer systems is illegal. Users must obtain proper authorization before use. The authors assume no liability for misuse or damage caused by this tool.

**By using CAP, you agree to use it responsibly and in compliance with all applicable laws.**

---

## Support & Contact

- **Issues:** [GitHub Issues](https://github.com/declanmiddleton/cap/issues)
- **Contributions:** Pull requests welcome
- **Documentation:** See `examples/` directory for usage guides

---

*CAP — Because time spent upgrading shells is time not spent finding vulnerabilities.*
