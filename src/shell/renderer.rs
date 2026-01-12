use anyhow::Result;
use crossterm::{
    cursor,
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, 
        EnterAlternateScreen, LeaveAlternateScreen,
        Clear, ClearType, size,
    },
};
use std::io::{self, Write};

/// Central terminal renderer with exclusive ownership
/// Only ONE renderer can be active at a time
pub struct TerminalRenderer {
    in_alternate_screen: bool,
    in_raw_mode: bool,
}

impl TerminalRenderer {
    pub fn new() -> Self {
        Self {
            in_alternate_screen: false,
            in_raw_mode: false,
        }
    }

    /// Enter alternate screen buffer - guarantees clean slate
    pub fn enter_alternate_screen(&mut self) -> Result<()> {
        if !self.in_alternate_screen {
            execute!(io::stdout(), EnterAlternateScreen)?;
            execute!(io::stdout(), Clear(ClearType::All))?;
            execute!(io::stdout(), cursor::MoveTo(0, 0))?;
            self.in_alternate_screen = true;
        }
        Ok(())
    }

    /// Leave alternate screen buffer - return to previous content
    pub fn leave_alternate_screen(&mut self) -> Result<()> {
        if self.in_alternate_screen {
            execute!(io::stdout(), LeaveAlternateScreen)?;
            self.in_alternate_screen = false;
        }
        Ok(())
    }

    /// Enable raw mode for direct terminal control
    pub fn enable_raw_mode(&mut self) -> Result<()> {
        if !self.in_raw_mode {
            enable_raw_mode()?;
            self.in_raw_mode = true;
        }
        Ok(())
    }

    /// Disable raw mode
    pub fn disable_raw_mode(&mut self) -> Result<()> {
        if self.in_raw_mode {
            disable_raw_mode()?;
            self.in_raw_mode = false;
        }
        Ok(())
    }

    /// Clear the entire screen and reset cursor
    pub fn clear_screen(&self) -> Result<()> {
        execute!(io::stdout(), Clear(ClearType::All))?;
        execute!(io::stdout(), cursor::MoveTo(0, 0))?;
        Ok(())
    }

    /// Get current terminal dimensions
    pub fn get_size(&self) -> Result<(u16, u16)> {
        Ok(size()?)
    }

    /// Flush output buffer
    pub fn flush(&self) -> Result<()> {
        io::stdout().flush()?;
        Ok(())
    }

    /// Full cleanup - restore terminal to normal state
    pub fn cleanup(&mut self) -> Result<()> {
        self.disable_raw_mode()?;
        self.leave_alternate_screen()?;
        self.clear_screen()?;
        self.flush()?;
        Ok(())
    }

    /// Transition to menu mode (alternate screen + raw mode)
    pub fn transition_to_menu(&mut self) -> Result<()> {
        self.enter_alternate_screen()?;
        self.enable_raw_mode()?;
        self.clear_screen()?;
        Ok(())
    }

    /// Transition to session mode (main screen + raw mode)
    pub fn transition_to_session(&mut self) -> Result<()> {
        self.leave_alternate_screen()?;
        self.enable_raw_mode()?;
        // Don't clear - preserve session output
        Ok(())
    }

    /// Transition to listening mode (main screen + no raw mode)
    pub fn transition_to_listening(&mut self) -> Result<()> {
        self.leave_alternate_screen()?;
        self.disable_raw_mode()?;
        self.clear_screen()?;
        Ok(())
    }
}

impl Drop for TerminalRenderer {
    fn drop(&mut self) {
        // Ensure cleanup on drop
        let _ = self.cleanup();
    }
}

/// Output buffer for storing session output while menu is active
pub struct OutputBuffer {
    buffer: Vec<String>,
    max_size: usize,
}

impl OutputBuffer {
    pub fn new(max_size: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(max_size),
            max_size,
        }
    }

    /// Store output (called when menu is active)
    pub fn push(&mut self, output: String) {
        if self.buffer.len() >= self.max_size {
            // Remove oldest to make room
            self.buffer.remove(0);
        }
        self.buffer.push(output);
    }

    /// Flush all buffered output to stdout
    pub fn flush_to_stdout(&mut self) -> Result<()> {
        for line in self.buffer.drain(..) {
            print!("{}", line);
        }
        io::stdout().flush()?;
        Ok(())
    }

    /// Check if buffer has content
    pub fn has_content(&self) -> bool {
        !self.buffer.is_empty()
    }

    /// Clear buffer without flushing
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_buffer() {
        let mut buffer = OutputBuffer::new(100);
        buffer.push("test1".to_string());
        buffer.push("test2".to_string());
        assert!(buffer.has_content());
        assert_eq!(buffer.buffer.len(), 2);
    }

    #[test]
    fn test_output_buffer_max_size() {
        let mut buffer = OutputBuffer::new(2);
        buffer.push("1".to_string());
        buffer.push("2".to_string());
        buffer.push("3".to_string()); // Should remove "1"
        assert_eq!(buffer.buffer.len(), 2);
        assert_eq!(buffer.buffer[0], "2");
        assert_eq!(buffer.buffer[1], "3");
    }
}
