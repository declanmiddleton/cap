use anyhow::Result;
use colored::Colorize;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
};
use std::io::{self, Write};
use std::net::IpAddr;

const PRIMARY_COLOR: Color = Color::Rgb { r: 37, g: 150, b: 190 };
const SECONDARY_COLOR: Color = Color::Rgb { r: 86, g: 33, b: 213 };
const MUTED_COLOR: Color = Color::Rgb { r: 120, g: 120, b: 130 };

pub struct InterfaceSelector {
    interfaces: Vec<(String, IpAddr)>,
    selected: usize,
}

impl InterfaceSelector {
    pub fn new() -> Result<Self> {
        let interfaces = Self::get_network_interfaces()?;
        Ok(Self {
            interfaces,
            selected: 0,
        })
    }

    fn get_network_interfaces() -> Result<Vec<(String, IpAddr)>> {
        let mut interfaces = Vec::new();
        
        // Always add "all interfaces" option first
        interfaces.push(("all interfaces".to_string(), "0.0.0.0".parse()?));
        
        // Enumerate ALL network interfaces using if-addrs
        if let Ok(addrs) = if_addrs::get_if_addrs() {
            for iface in addrs {
                let ip = iface.addr.ip();
                
                // Skip IPv6 for now (can be added later)
                if ip.is_ipv6() {
                    continue;
                }
                
                // Categorize interfaces
                let name = if ip.is_loopback() {
                    format!("localhost ({})", iface.name)
                } else if iface.name.starts_with("tun") || iface.name.starts_with("tap") {
                    format!("vpn/{} ({})", iface.name, determine_vpn_type(&iface.name))
                } else if iface.name.starts_with("eth") || iface.name.starts_with("en") {
                    format!("ethernet ({})", iface.name)
                } else if iface.name.starts_with("wl") || iface.name.starts_with("wi") {
                    format!("wireless ({})", iface.name)
                } else if iface.name.starts_with("docker") || iface.name.starts_with("br-") {
                    format!("docker ({})", iface.name)
                } else {
                    format!("{}", iface.name)
                };
                
                // Avoid duplicates
                if !interfaces.iter().any(|(_, existing_ip)| existing_ip == &ip) {
                    interfaces.push((name, ip));
                }
            }
        }
        
        // If no interfaces found (fallback), add localhost
        if interfaces.len() == 1 {
            interfaces.push(("localhost".to_string(), "127.0.0.1".parse()?));
        }
        
        Ok(interfaces)
    }

    fn print_colored(&self, text: &str, color: Color) {
        let _ = execute!(io::stdout(), SetForegroundColor(color), Print(text), ResetColor);
    }

    pub async fn select(&mut self) -> Result<String> {
        println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".truecolor(37, 150, 190));
        self.print_colored("  Interface Selection", PRIMARY_COLOR);
        println!();
        println!("{}", "  Use ↑↓ or Tab to select · Enter to confirm · Ctrl+C to cancel".truecolor(120, 120, 130));
        println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".truecolor(37, 150, 190));
        println!();
        
        enable_raw_mode()?;
        let result = self.run_selector().await;
        disable_raw_mode()?;
        
        result
    }

    async fn run_selector(&mut self) -> Result<String> {
        loop {
            print!("\r");
            execute!(io::stdout(), cursor::MoveUp(self.interfaces.len() as u16))?;
            
            for (idx, (name, ip)) in self.interfaces.iter().enumerate() {
                print!("\r");
                execute!(io::stdout(), Clear(ClearType::CurrentLine))?;
                
                if idx == self.selected {
                    self.print_colored("  ◉ ", PRIMARY_COLOR);
                    self.print_colored(&format!("{:35}", name), PRIMARY_COLOR);
                    self.print_colored(&format!("{}", ip), SECONDARY_COLOR);
                } else {
                    self.print_colored("  ◦ ", MUTED_COLOR);
                    print!("{:35}", name.truecolor(100, 100, 110));
                    print!("{}", ip.to_string().truecolor(80, 80, 90));
                }
                println!();
            }
            
            io::stdout().flush()?;
            
            if event::poll(std::time::Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Tab | KeyCode::Down | KeyCode::Char('j') => {
                            self.selected = (self.selected + 1) % self.interfaces.len();
                        }
                        KeyCode::BackTab | KeyCode::Up | KeyCode::Char('k') => {
                            self.selected = if self.selected == 0 {
                                self.interfaces.len() - 1
                            } else {
                                self.selected - 1
                            };
                        }
                        KeyCode::Enter => {
                            let selected_ip = self.interfaces[self.selected].1.to_string();
                            
                            // Clear interface list
                            for _ in 0..self.interfaces.len() {
                                execute!(io::stdout(), cursor::MoveUp(1), Clear(ClearType::CurrentLine))?;
                            }
                            
                            // Show selected IP
                            self.print_colored("  ◉ ", PRIMARY_COLOR);
                            self.print_colored("Interface  ", PRIMARY_COLOR);
                            self.print_colored(&selected_ip, SECONDARY_COLOR);
                            println!("\n");
                            
                            return Ok(selected_ip);
                        }
                        KeyCode::Esc | KeyCode::Char('q') => {
                            return Err(anyhow::anyhow!("Selection cancelled"));
                        }
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            return Err(anyhow::anyhow!("Selection cancelled"));
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

fn determine_vpn_type(iface_name: &str) -> &str {
    if iface_name.contains("tun") {
        if iface_name.contains("0") || iface_name == "tun0" {
            "openvpn"
        } else {
            "tunnel"
        }
    } else if iface_name.contains("tap") {
        "tap"
    } else {
        "vpn"
    }
}
