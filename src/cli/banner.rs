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
    
    // Central Access Point
    println!("{}", "  Central Access Point".truecolor(86, 33, 213));
    println!("{}", "  Shell handler built for reliability, clarity, and flow\n".truecolor(120, 120, 130));
}
