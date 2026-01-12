use anyhow::Result;
use colored::Colorize;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    style::{Color, Print, ResetColor, SetForegroundColor},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
};
use std::io::{self, Write};

const PRIMARY_COLOR: Color = Color::Rgb { r: 37, g: 150, b: 190 };
const SECONDARY_COLOR: Color = Color::Rgb { r: 86, g: 33, b: 213 };
const MUTED_COLOR: Color = Color::Rgb { r: 120, g: 120, b: 130 };

fn print_colored(text: &str, color: Color) {
    let _ = execute!(io::stdout(), SetForegroundColor(color), Print(text), ResetColor);
}

pub fn get_port_input() -> Result<u16> {
    // Port appears directly under IP
    print_colored("  ◉ ", PRIMARY_COLOR);
    print_colored("Port       ", PRIMARY_COLOR);
    print_colored("4444", SECONDARY_COLOR);
    print!("  ");
    print_colored("Enter", MUTED_COLOR);
    print!(" to confirm");
    io::stdout().flush()?;
    
    enable_raw_mode()?;
    
    let mut input = String::from("4444");
    let mut cursor_pos = input.len();
    
    loop {
        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Enter => {
                        disable_raw_mode()?;
                        println!("\n");
                        
                        match input.parse::<u16>() {
                            Ok(port) if port > 0 => {
                                return Ok(port);
                            }
                            _ => {
                                print_colored("  ◉ ", PRIMARY_COLOR);
                                println!("Invalid port, using 4444\n");
                                return Ok(4444);
                            }
                        }
                    }
                    KeyCode::Char(c) if c.is_ascii_digit() && input.len() < 5 => {
                        input.insert(cursor_pos, c);
                        cursor_pos += 1;
                        
                        // Redraw
                        print!("\r");
                        execute!(io::stdout(), Clear(ClearType::CurrentLine))?;
                        print_colored("  ◉ ", PRIMARY_COLOR);
                        print_colored("Port       ", PRIMARY_COLOR);
                        print_colored(&input, SECONDARY_COLOR);
                        print!("  ");
                        print_colored("Enter", MUTED_COLOR);
                        print!(" to confirm");
                        io::stdout().flush()?;
                    }
                    KeyCode::Backspace => {
                        if cursor_pos > 0 && !input.is_empty() {
                            cursor_pos -= 1;
                            input.remove(cursor_pos);
                            
                            // Redraw
                            print!("\r");
                            execute!(io::stdout(), Clear(ClearType::CurrentLine))?;
                            print_colored("  ◉ ", PRIMARY_COLOR);
                            print_colored("Port       ", PRIMARY_COLOR);
                            print_colored(&input, SECONDARY_COLOR);
                            print!("  ");
                            print_colored("Enter", MUTED_COLOR);
                            print!(" to confirm");
                            io::stdout().flush()?;
                        }
                    }
                    KeyCode::Left => {
                        if cursor_pos > 0 {
                            cursor_pos -= 1;
                        }
                    }
                    KeyCode::Right => {
                        if cursor_pos < input.len() {
                            cursor_pos += 1;
                        }
                    }
                    KeyCode::Esc | KeyCode::Char('q') => {
                        disable_raw_mode()?;
                        return Err(anyhow::anyhow!("Input cancelled"));
                    }
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        disable_raw_mode()?;
                        return Err(anyhow::anyhow!("Input cancelled"));
                    }
                    _ => {}
                }
            }
        }
    }
}
