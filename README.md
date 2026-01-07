<img width="1056" height="357" alt="image" src="https://github.com/user-attachments/assets/d2359001-218c-47f1-aa99-a00432006854" />



## Description & Purpose

CAP (Comprehensive Assessment Platform) is a modern, Rust-based security assessment and orchestration framework designed for authorized penetration testing, defensive research, and security training.

CAP provides a unified interface for reconnaissance, session handling, and controlled shell interaction, with a strong emphasis on scope enforcement, auditability, and operational safety.
Unlike traditional C2 frameworks, CAP is intentionally opinionated: all actions are scoped, logged, and attributable.

The framework is suitable for use during authorized post-exploitation phases, controlled red team engagements, blue team validation, lab environments, and educational use cases.

CAP is built from the ground up in Rust, prioritizing memory safety, performance, and transparency, while still offering the practical ergonomics expected from modern security tooling.

## What CAP Is (and Is Not)
CAP is:
- A security assessment framework
- A controlled shell listener
- A reconnaissance and enumeration orchestrator
- A research and training framework
- An exploitation toolkit
  
CAP is not:
- A malware framework
  
Typical scenarios include:
- Authorized penetration tests
- Red team reconnaissance and post-exploitation
- Security research and tool development
- CTFs and lab environments


## Installation
# Linux
On most Linux distributions, Rust can be installed either through the system package manager or via the official Rust installer. On Debian-based systems such as Ubuntu, Pop!_OS, Kali Linux, and Parrot OS, Rust and Cargo can be installed using:

```bash
sudo apt update
sudo apt install cargo rustc
```

On Arch Linux and Arch-based distributions, Rust is available from the official repositories and can be installed using:

```bash
sudo pacman -S rust cargo
```

If Kerberos-authenticated functionality is required, the Kerberos client package must also be installed. On Debian-based systems this package is typically named `krb5-user`, while on Arch-based systems it is provided by `krb5`.

Once Rust is installed, CAP can be built from source by cloning the repository and compiling it with Cargo:

```bash
git clone https://github.com/yourusername/cap.git
cd cap
cargo build --release
```

The compiled binary will be available at `target/release/cap` and can be executed directly or moved into a directory included in the system PATH.


# Static Linux Build
CAP can be built as a statically linked binary for portability across Linux systems. This is useful when transferring the binary between machines or running it in minimal environments.
To build a static binary using musl, first install the musl target:
```bash
rustup target add x86_64-unknown-linux-musl
```
On Debian-based systems, the musl toolchain can be installed with:
```bash
sudo apt install musl-tools
```
On Arch Linux, the musl toolchain is available via:
```bash
sudo pacman -S musl
```
Once installed, build the static binary using:
```bash
cargo build --release --target x86_64-unknown-linux-musl
```
The resulting static binary will be located at:
```text
target/x86_64-unknown-linux-musl/release/cap
```
This binary can be copied and executed on compatible Linux systems without additional runtime dependencies.

# macOS
On macOS, Rust can be installed using Homebrew or the official Rust installer. If Homebrew is installed, Rust can be installed with:

```bash
brew install rust
```

Alternatively, the official installer can be used:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

After installing Rust, CAP can be built normally using Cargo:

```bash
git clone https://github.com/yourusername/cap.git
cd cap
cargo build --release
```

The resulting binary will be available at `target/release/cap`. macOS static linking is not supported in the same way as Linux, but the release binary is fully self-contained for the target system.

### Windows
On Windows, Rust can be installed using the official Rust installer from [https://rustup.rs](https://rustup.rs). During installation, select the MSVC toolchain when prompted.
Once Rust and Cargo are installed, CAP can be built from a standard command prompt or PowerShell:

```powershell
git clone https://github.com/yourusername/cap.git
cd cap
cargo build --release
```

The compiled executable will be located at:

```text
target\release\cap.exe
```

Windows Defender or other endpoint security products may flag security tooling binaries. If this occurs in a lab or authorized environment, appropriate exclusions may be required.

### Notes
CAP relies only on standard terminal capabilities for its interactive features. Command history, tab completion, and session navigation are supported on modern terminals without additional configuration. All builds produce a single binary that can be executed directly without runtime dependencies. This tool remains in development and will continue to remain open-source to viewing. 

