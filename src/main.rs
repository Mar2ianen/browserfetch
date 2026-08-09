mod browser;
mod extensions;
mod logo;
mod os;
mod output;
mod util;

use browser::Browser;

fn main() {
    if std::env::args().any(|arg| arg == "-h" || arg == "--help") {
        print_help();
        return;
    }

    let browser = Browser::detect();
    output::render(&browser);
}

fn print_help() {
    println!("browserfetch - fastfetch-style summary for the default browser");
    println!();
    println!("Usage: browserfetch");
    println!();
    println!("Detects the default XDG browser and prints browser, OS and extension info.");
    println!("Uses chafa for browser logos. If chafa/icon is unavailable, prints a text label.");
}
