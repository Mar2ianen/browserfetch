mod browser;
mod extensions;
mod logo;
mod os;
mod output;
mod util;

use browser::Browser;

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_help();
        return;
    }

    if args.first().is_some_and(|arg| arg == "--list") {
        print_installed_browsers();
        return;
    }

    if args.len() > 1 {
        eprintln!("Expected at most one browser selector.");
        print_help();
        std::process::exit(2);
    }

    let browser = match args.first() {
        Some(selector) => Browser::detect_selector(selector).unwrap_or_else(|| {
            eprintln!("No installed browser matched: {selector}");
            print_installed_browsers();
            std::process::exit(2);
        }),
        None => Browser::detect(),
    };
    output::render(&browser);
}

fn print_help() {
    println!("browserfetch - fastfetch-style summary for the default browser");
    println!();
    println!("Usage: browserfetch [browser]");
    println!("       browserfetch --list");
    println!();
    println!("Without a selector, uses the default XDG browser.");
    println!("A selector chooses an installed browser by name, desktop ID or executable.");
    println!("Prints browser, engine, OS, profile and extension info.");
    println!("Uses chafa for browser logos. If chafa/icon is unavailable, prints a text label.");
}

fn print_installed_browsers() {
    let browsers = Browser::installed_browsers();
    if browsers.is_empty() {
        println!("No browser desktop entries found.");
        return;
    }

    println!("Installed browser entries:");
    for (name, id) in browsers {
        println!("  {name} ({id})");
    }
}
