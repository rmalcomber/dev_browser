//! The Dev Browser binary. Wiring only: the command line, the data folder, and starting the host.

use dev_browser_core::{data::DataDir, workspace};
use dev_browser_host::Config;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("dev_browser {}", dev_browser_host::VERSION);
        return;
    }
    // CEF starts its child processes (renderer, GPU, ...) from this same executable with --type=...; they go straight to the host and
    // must not touch the data folder.
    if args.iter().any(|a| a.starts_with("--type=")) {
        std::process::exit(dev_browser_host::run(child_config()));
    }
    match start(&args) {
        Ok(config) => std::process::exit(dev_browser_host::run(config)),
        Err(e) => {
            eprintln!("dev_browser: {e}");
            std::process::exit(1);
        }
    }
}

/// Child processes never read the config; this only satisfies the type, without creating anything on disk.
fn child_config() -> Config {
    Config {
        data: DataDir::unopened(std::env::temp_dir()),
        profile_dir: PathBuf::new(),
        start_url: String::new(),
    }
}

fn start(args: &[String]) -> Result<Config, String> {
    let flag = |name: &str| {
        args.iter()
            .find_map(|a| a.strip_prefix(&format!("--{name}=")).map(str::to_owned))
    };
    let root = flag("data-dir")
        .map(PathBuf::from)
        .or_else(DataDir::default_location)
        .ok_or("cannot find a data folder (pass --data-dir=PATH)")?;
    let data = DataDir::open(&root).map_err(|e| e.to_string())?;
    let mut global = data.open_global().map_err(|e| e.to_string())?;
    // M1.2 adds real workspace management; until then there is one workspace, created on first run.
    let first = match workspace::list(&global)
        .map_err(|e| e.to_string())?
        .into_iter()
        .next()
    {
        Some(w) => w,
        None => data
            .create_workspace(&mut global, "Default", 0x2C5FA8)
            .map_err(|e| e.to_string())?,
    };
    drop(global);
    data.open_workspace(&first.id).map_err(|e| e.to_string())?;
    Ok(Config {
        profile_dir: data.chromium_profile(&first.id),
        start_url: flag("url").unwrap_or_else(|| "https://example.com/".to_string()),
        data,
    })
}
