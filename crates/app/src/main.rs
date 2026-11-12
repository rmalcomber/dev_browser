//! The Dev Browser binary. Wiring only: the command line, logging, and starting the host. Nothing is wired yet (M1.1).

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_default();
    match arg.as_str() {
        "--version" | "-V" => println!("dev_browser {}", dev_browser_host::VERSION),
        _ => {
            eprintln!(
                "dev_browser {}: nothing to run yet (M1.1 adds the window). Try --version.",
                dev_browser_host::VERSION
            );
            std::process::exit(2);
        }
    }
}
