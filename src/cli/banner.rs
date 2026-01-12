use colored::Colorize;

/// Display CAP banner (for help/main command only)
pub fn display_banner() {
    let banner = r#"
 ▄████▄   ▄▄▄       ██▓███  
▒██▀ ▀█  ▒████▄    ▓██░  ██▒
▒▓█    ▄ ▒██  ▀█▄  ▓██░ ██▓▒
▒▓▓▄ ▄██▒░██▄▄▄▄██ ▒██▄█▓▒ ▒
▒ ▓███▀ ░ ▓█   ▓██▒▒██▒ ░  ░
░ ░▒ ▒  ░ ▒▒   ▓▒█░▒▓▒░ ░  ░
  ░  ▒     ▒   ▒▒ ░░▒ ░     
░          ░   ▒   ░░       
░ ░            ░  ░         
░                           
    "#;

    // Primary color #2596be (RGB: 37, 150, 190)
    println!("{}", banner.truecolor(37, 150, 190));
    
    // Secondary color #5621d5 (RGB: 86, 33, 213)
    println!("{}", "  Modern terminal-based reverse shell handler".truecolor(86, 33, 213));
    
    // Muted color for authorization notice
    println!("{}", "  Authorized use only\n".truecolor(120, 120, 130));
}
