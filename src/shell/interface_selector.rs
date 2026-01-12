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
        use std::net::UdpSocket;
        
        let mut interfaces = Vec::new();
        
        // Add localhost
        interfaces.push(("lo (localhost)".to_string(), "127.0.0.1".parse()?));
        
        // Add 0.0.0.0 (all interfaces)
        interfaces.push(("all (0.0.0.0)".to_string(), "0.0.0.0".parse()?));
        
        // Try to get actual interface IP
        // This is a simple heuristic - connect to external address to determine route
        if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
            if socket.connect("8.8.8.8:80").is_ok() {
                if let Ok(addr) = socket.local_addr() {
                    let ip = addr.ip();
                    if !ip.is_loopback() && !ip.to_string().starts_with("0.0.0.0") {
                        interfaces.push((format!("primary ({})", ip), ip));
                    }
                }
            }
        }
        
        // Add common private network interfaces
        for (name, ip_str) in &[
            ("tun0 (VPN)", "10.10.14.1"),
            ("eth0", "192.168.1.100"),
            ("wlan0", "192.168.1.101"),
        ] {
            if let Ok(ip) = ip_str.parse() {
                // Only add if not duplicate
                if !interfaces.iter().any(|(_, existing_ip)| existing_ip == &ip) {
                    interfaces.push((name.to_string(), ip));
                }
            }
        }
        
        Ok(interfaces)
    }

    fn print_colored(&self, text: &str, color: Color) {
        let _ = execute!(io::stdout(), SetForegroundColor(color), Print(text), ResetColor);
    }

    pub async fn select(&mut self) -> Result<String> {
        println!();
        self.print_colored("◉ ", PRIMARY_COLOR);
        println!("Select interface\n");
        
        enable_raw_mode()?;
        
        let result = self.run_selector().await;
        
        disable_raw_mode()?;
        
        result
    }

    async fn run_selector(&mut self) -> Result<String> {
        loop {
            // Clear previous display
            print!("\r");
            execute!(io::stdout(), cursor::MoveUp(self.interfaces.len() as u16))?;
            
            // Display all interfaces
            for (idx, (name, ip)) in self.interfaces.iter().enumerate() {
                print!("\r");
                execute!(io::stdout(), Clear(ClearType::CurrentLine))?;
                
                if idx == self.selected {
                    // Selected interface
                    self.print_colored("  ◉ ", PRIMARY_COLOR);
                    self.print_colored(&format!("{:20}", name), PRIMARY_COLOR);
                    self.print_colored(&format!(" {}", ip), SECONDARY_COLOR);
                } else {
                    // Unselected interface
                    self.print_colored("  ◦ ", MUTED_COLOR);
                    print!("{:20} ", name.truecolor(120, 120, 130));
                    print!("{}", ip.to_string().truecolor(100, 100, 110));
                }
                println!();
            }
            
            io::stdout().flush()?;
            
            // Handle input
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
                            
                            // Clear the selector display
                            for _ in 0..self.interfaces.len() {
                                execute!(io::stdout(), cursor::MoveUp(1), Clear(ClearType::CurrentLine))?;
                            }
                            
                            // Show confirmation
                            print!("\r");
                            self.print_colored("◉ ", PRIMARY_COLOR);
                            print!("Interface: ");
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
