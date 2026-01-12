use crossterm::terminal::size;
use std::io;

/// Get current terminal dimensions
pub fn get_terminal_size() -> io::Result<(u16, u16)> {
    size()
}

/// Wrap text to fit within a maximum width, respecting word boundaries
pub fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if text.len() <= max_width {
        return vec![text.to_string()];
    }
    
    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0;
    
    for word in text.split_whitespace() {
        let word_len = word.len();
        
        // Check if adding this word would exceed max width
        let space_needed = if current_width == 0 { 0 } else { 1 }; // Space before word
        
        if current_width + space_needed + word_len > max_width {
            if !current_line.is_empty() {
                lines.push(current_line.clone());
                current_line.clear();
                current_width = 0;
            }
            
            // If single word is longer than max_width, break it
            if word_len > max_width {
                let mut remaining = word;
                while remaining.len() > max_width {
                    lines.push(remaining[..max_width].to_string());
                    remaining = &remaining[max_width..];
                }
                if !remaining.is_empty() {
                    current_line = remaining.to_string();
                    current_width = remaining.len();
                }
            } else {
                current_line = word.to_string();
                current_width = word_len;
            }
        } else {
            if current_width > 0 {
                current_line.push(' ');
                current_width += 1;
            }
            current_line.push_str(word);
            current_width += word_len;
        }
    }
    
    if !current_line.is_empty() {
        lines.push(current_line);
    }
    
    if lines.is_empty() {
        lines.push(String::new());
    }
    
    lines
}

/// Truncate text to fit within max width, adding ellipsis if needed
pub fn truncate_text(text: &str, max_width: usize) -> String {
    if text.len() <= max_width {
        text.to_string()
    } else if max_width < 3 {
        text[..max_width].to_string()
    } else {
        format!("{}...", &text[..max_width - 3])
    }
}

/// Format text in two columns with proper alignment and wrapping
pub fn format_two_columns(
    left: &str,
    right: &str,
    left_width: usize,
    total_width: usize,
    indent: usize,
) -> Vec<String> {
    let mut lines = Vec::new();
    
    // Calculate right column width
    let spacing = 3; // Space between columns
    let right_max_width = if total_width > indent + left_width + spacing {
        total_width - indent - left_width - spacing
    } else {
        20 // Minimum width for right column
    };
    
    // Wrap right column text
    let right_lines = wrap_text(right, right_max_width);
    
    // First line includes left text
    let left_truncated = truncate_text(left, left_width);
    let first_line = format!(
        "{:indent$}{:left_width$}   {}",
        "",
        left_truncated,
        right_lines.get(0).unwrap_or(&String::new()),
        indent = indent,
        left_width = left_width
    );
    lines.push(first_line);
    
    // Subsequent lines only have right column (indented)
    for line in right_lines.iter().skip(1) {
        let continued_line = format!(
            "{:indent$}{:left_width$}   {}",
            "",
            "",
            line,
            indent = indent,
            left_width = left_width
        );
        lines.push(continued_line);
    }
    
    lines
}

/// Ensure a line doesn't exceed terminal width
pub fn constrain_line(line: &str, max_width: usize) -> String {
    truncate_text(line, max_width)
}

/// Get safe terminal width (with margin)
pub fn get_safe_width() -> usize {
    match get_terminal_size() {
        Ok((cols, _)) => {
            // Leave 2 character margin on right
            if cols > 2 {
                (cols - 2) as usize
            } else {
                80 // Fallback
            }
        }
        Err(_) => 80, // Default fallback
    }
}

/// Center text within given width
pub fn center_text(text: &str, width: usize) -> String {
    if text.len() >= width {
        return text.to_string();
    }
    
    let padding = (width - text.len()) / 2;
    format!("{:padding$}{}", "", text, padding = padding)
}

/// Create a horizontal line of a specific character
pub fn horizontal_line(width: usize, ch: char) -> String {
    ch.to_string().repeat(width)
}

/// Alias for wrap_text for compatibility
pub fn word_wrap(text: &str, max_width: usize) -> Vec<String> {
    wrap_text(text, max_width)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_text() {
        let text = "This is a long line that needs to be wrapped";
        let lines = wrap_text(text, 20);
        assert!(lines.len() > 1);
        for line in lines {
            assert!(line.len() <= 20);
        }
    }

    #[test]
    fn test_truncate_text() {
        assert_eq!(truncate_text("Hello", 10), "Hello");
        assert_eq!(truncate_text("Hello World", 8), "Hello...");
        assert_eq!(truncate_text("Hi", 5), "Hi");
    }

    #[test]
    fn test_format_two_columns() {
        let lines = format_two_columns("cmd", "Description here", 10, 50, 2);
        assert!(!lines.is_empty());
        assert!(lines[0].contains("cmd"));
        assert!(lines[0].contains("Description"));
    }
}
