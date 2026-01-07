use colored::Colorize;

/// Display CAP banner (for help/main command only)
pub fn display_banner() {
    let banner = r#"
 ______     ______     ______  
/\  ___\   /\  __ \   /\  == \ 
\ \ \____  \ \  __ \  \ \  _-/ 
 \ \_____\  \ \_\ \_\  \ \_\   
  \/_____/   \/_/\/_/   \/_/   
                               
    "#;

    println!("{}", banner.truecolor(255, 140, 0));
    println!("{}", "  CAP – Comprehensive Assessment Platform".bright_yellow());
    println!("{}", "  Security assessment tool for authorized penetration testing".bright_black());
    println!();
    println!("{}", "  Authorized use only. Ensure you have permission before testing any target.".yellow());
    println!();
}

