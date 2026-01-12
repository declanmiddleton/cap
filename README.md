# CAP

**Central Access Point** — A modern reverse shell handler built for reliability, clarity, and flow.

<img width="1056" height="357" alt="CAP Banner" src="https://github.com/user-attachments/assets/d2359001-218c-47f1-aa99-a00432006854" />

---

## What is CAP?

CAP is a **terminal-based reverse shell handler** that makes remote interaction feel stable, calm, and local. It strips away unnecessary complexity, focusing entirely on session capture, management, and transparent interaction.

### Design Philosophy

**Shell-first.** CAP is intentionally narrow in scope—it's a listener and session manager, not a full framework. Once a connection is received, the shell is immediately stabilized and made interactive with consistent input/output behavior.

**Clarity over clutter.** No modes, no menus you don't need, no command-heavy workflows. Interaction begins directly in the shell and remains uninterrupted. Sessions maintain continuous context—target identity, privilege level, session age, and operator notes—always visible, never hidden.

**Built to disappear.** Shell detection, stabilization, reconnection, and recovery happen automatically and silently. The tool gets out of your way so you can focus on the engagement, not the tool itself.

---

## Key Features

### 🎯 **Modern Shell Management**
- **Interactive listener** with guided interface and port selection
- **Multi-session handling** — capture, background, attach, and switch between shells seamlessly
- **Persistent context** — target hostname, username, privilege level, and session age displayed in every prompt
- **Session notes** — annotate shells for tracking and organization
- **Graceful recovery** — sessions survive disconnects and attempt automatic reconnection

### ⚡ **True Terminal Passthrough**
- **Byte-for-byte forwarding** — keystrokes sent as raw bytes, zero interpretation
- **Async I/O** — concurrent stdin/stdout processing with tokio
- **Raw mode** — character-by-character input, remote shell handles line editing
- **No local echo** — remote shell provides echo for natural behavior
- **Full PTY support** — arrow keys, backspace, Ctrl+C, tab completion all work correctly

### 🎨 **Restrained, Modern Interface**
- **Two-color system** — `#2596be` (primary) and `#5621d5` (secondary) for consistent, calm aesthetics
- **Subtle animations** — brief feedback for state changes without distraction
- **Terminal-aware rendering** — dynamic width detection, smart text wrapping, no overflow
- **Clean state management** — strict separation between listener, menu, and interactive shell modes

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
./target/release/cap
```

### Basic Usage

```bash
# Start listener (interactive setup - default)
cap

# Or specify host and port directly
cap listen --host 10.10.14.5 --port 4444

# List active sessions
cap sessions

# Attach to a session
cap attach <session-id>

# Add notes to a session
cap note <session-id> "Domain Admin shell - DC01"

# Kill a session
cap kill <session-id>
```

### Interactive Mode

Once a session is captured:

- **Type naturally** — commands assemble character-by-character
- **Press Enter** to execute
- **Press Esc** to detach and return to menu (session stays alive)
- **Ctrl+C, Ctrl+Z, etc.** are forwarded to the remote shell

Sessions display persistent metadata:
```
nibbler@Nibbles (user, 2m 34s) [Active] # whoami
```

---

## How It Works

### Connection Flow

1. **Listener starts** — select interface and port via interactive prompt
2. **Connection received** — session captured and registered
3. **Automatic stabilization** — OS detection, user enumeration, PTY upgrade (silent)
4. **Interactive mode** — true terminal passthrough with raw mode enabled
5. **Detach with Esc** — session backgrounds, remains alive for re-attachment

### Technical Architecture

- **Tokio async runtime** — non-blocking I/O for concurrent session handling
- **Raw termios control** — manual raw mode setup for proper character forwarding
- **TCP passthrough** — direct stdin → socket → stdout bridging
- **State machine** — strict terminal ownership (Listening, InMenu, InShell)
- **Alternate screen** — menu rendered on alternate buffer, session screen preserved
- **Session persistence** — metadata saved to `shell_sessions.json`

---

## Platform Support

| Platform | Status | Notes |
|----------|--------|-------|
| **Linux** | ✅ Fully supported | Debian, Ubuntu, Arch, Kali, Parrot |
| **macOS** | ✅ Fully supported | Apple Silicon & Intel |
| **Windows** | ⚠️ Partial | Terminal features limited by Windows PTY support |

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

- 🔴 **Penetration Testing** — reliable shell management during assessments
- 🟣 **Red Team Operations** — maintain access with stable, recoverable sessions
- 🔵 **Blue Team Validation** — test detection capabilities
- 🧪 **Security Research** — rapid shell interaction and experimentation
- 🎓 **Training & Labs** — educational environments and CTFs

---

## What CAP Is Not

CAP is **not**:
- ❌ A full C2 framework (use Metasploit, Sliver, Empire for that)
- ❌ A post-exploitation module platform
- ❌ An evasion toolkit
- ❌ Designed for unauthorized access

**CAP is a reverse shell handler.** It captures connections, stabilizes shells, and provides a clean interface for interaction. Complexity is intentionally minimal.

**Use responsibly.** CAP is intended for authorized engagements only. Users are responsible for compliance with applicable laws and regulations.

---

## Technical Details

### Dependencies

```toml
tokio = "1.35"           # Async runtime
crossterm = "0.28"       # Terminal control
nix = "0.27"             # UNIX termios, raw mode
colored = "2.1"          # Terminal colors
chrono = "0.4"           # Timestamps
dashmap = "5.5"          # Concurrent session map
uuid = "1.6"             # Session IDs
```

### Architecture Highlights

- **Async stdin/stdout** — `tokio::io::stdin()` and `tokio::io::stdout()` for proper async I/O
- **Raw byte forwarding** — `write_raw_bytes()` bypasses command processing
- **Enter sends `\r` only** — remote shell handles line discipline
- **Terminal state restoration** — termios settings always restored on exit
- **Concurrent I/O** — `tokio::select!` enables simultaneous stdin read and stdout write

### Why Rust?

- **Memory safety** — no buffer overflows, use-after-free, or data races
- **Performance** — zero-cost abstractions, compiled binary
- **Concurrency** — tokio provides robust async I/O
- **Single binary** — static linking produces standalone executable
- **Reliability** — strong type system catches bugs at compile time

---

## Comparison to Similar Tools

| Feature | CAP | Penelope | Pwncat | Netcat |
|---------|-----|----------|--------|--------|
| **Interactive listener** | ✅ | ✅ | ✅ | ❌ |
| **Multi-session** | ✅ | ✅ | ✅ | ❌ |
| **Auto-stabilization** | ✅ | ✅ | ✅ | ❌ |
| **Session persistence** | ✅ | ❌ | ✅ | ❌ |
| **Async I/O** | ✅ (tokio) | ❌ | ✅ (asyncio) | ❌ |
| **True raw mode** | ✅ (termios) | ✅ | ✅ | ❌ |
| **Single binary** | ✅ (Rust) | ❌ (Python) | ❌ (Python) | ✅ |
| **Modules/plugins** | ❌ | ✅ | ✅ | ❌ |

**CAP's niche:** Modern, minimal, reliable. Inspired by Penelope's clean UX, but built in Rust for a standalone binary with professional-grade async I/O.

---

## Project Status

CAP is under **active development** and will remain **open-source**.

### Current Features
- ✅ Interactive listener with interface/port selection
- ✅ Multi-session management
- ✅ True terminal passthrough (byte-for-byte forwarding)
- ✅ Async I/O with tokio
- ✅ Session metadata and notes
- ✅ Clean, minimal UI
- ✅ Terminal-aware rendering

### Roadmap
- [ ] Session logging (all commands + output to file)
- [ ] File upload/download
- [ ] In-memory script execution (LinPEAS, linux-smart-enumeration)
- [ ] Multi-shell spawning (multiple PTY sessions per target)
- [ ] Auto-reconnection improvements
- [ ] TAB completion in menu

Contributions, bug reports, and feature requests are welcome.

---

## Installation Examples

### Linux (Debian/Ubuntu/Kali/Parrot)

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Clone and build
git clone https://github.com/declanmiddleton/cap.git
cd cap
cargo build --release

# Optional: install to system
sudo cp target/release/cap /usr/local/bin/
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
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/declanmiddleton/cap.git
cd cap
cargo build --release
```

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
- **Documentation:** See source code for implementation details

---

*CAP — A reverse shell handler that gets out of your way.*
