use colored::Colorize;

pub fn display_banner() {
    let banner = r#"
    ██████╗ █████╗ ████████╗
   ██╔════╝██╔══██╗██╔═══██║██║     
   ██║     ███████║██████╔╝
   ██║     ██╔══██║██╔═══╝ 
   ╚██████╗██║  ██║██║     
    ╚═════╝╚═╝  ╚═╝╚═╝     
    "#;

    // Use TrueColor for vibrant orange (RGB: 255, 140, 0)
    let banner_orange = banner.truecolor(255, 140, 0).to_string();
    println!("{}", banner_orange);

    let subtitle = "  Comprehensive Assessment Platform";
    let version = "v0.1.0";
    let tagline = "Research-Oriented Security Orchestration Framework";

    println!("{}", subtitle.bright_yellow());
    println!(
        "  {} | {}\n",
        version.bright_black(),
        tagline.bright_blue()
    );

    let warning = "  ⚠  AUTHORIZED USE ONLY - For research, training, and approved testing";
    println!("{}", warning.yellow());
    println!(
        "  {} Ensure proper authorization before any assessment\n",
        "📋".to_string()
    );
}

