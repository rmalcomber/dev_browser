//! CEF integration for The Dev Browser: the only crate that touches CEF. All browser logic belongs in `dev_browser_core`; this crate
//! executes what core decides. One UI thread, handlers split by concern (see planning/m1-plan.md, section 4).

mod app;
mod handlers;
mod ipc;
mod keys;
mod layout;
mod pages;
mod profile;
mod render;
mod shell;
mod state;
mod window;

pub use state::Config;

/// The version of the host crate, shown by `dev_browser --version`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Run the browser until the window is closed. Returns the process exit code. Call once, from `main`, before anything else touches CEF.
/// Child processes (renderer, GPU, ...) are the same executable: for them this returns their exit code straight away.
pub fn run(config: Config) -> i32 {
    use cef::*;
    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);
    let args = cef::args::Args::new();
    let Some(cmd) = args.as_cmd_line() else {
        eprintln!("[host] no command line");
        return 1;
    };
    let is_browser = cmd.has_switch(Some(&CefString::from("type"))) != 1;
    let mut app = app::HostApp::new();
    let code = execute_process(
        Some(args.as_main_args()),
        Some(&mut app),
        std::ptr::null_mut(),
    );
    if !is_browser {
        return code.max(0);
    }
    if code != -1 {
        eprintln!("[host] unexpected execute_process result {code} in the browser process");
        return 1;
    }

    let root = config.data.chromium_root();
    let settings = Settings {
        no_sandbox: 1,
        log_severity: LogSeverity::WARNING,
        log_file: CefString::from(
            config
                .data
                .root()
                .join("cef.log")
                .to_string_lossy()
                .as_ref(),
        ),
        root_cache_path: CefString::from(root.to_string_lossy().as_ref()),
        ..Default::default()
    };
    // Open the backend (global database, first workspace, orphan sweep) before CEF starts: nothing holds the profile folders yet, so the
    // sweep can remove what a locked delete left behind last time.
    let backend =
        match dev_browser_core::backend::Backend::open(config.data.clone(), &config.start_url) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("[host] cannot open the data folder: {e}");
                return 1;
            }
        };
    for id in backend.orphans_removed() {
        println!("[host] removed leftover data of deleted workspace {id}");
    }
    state::with(|s| {
        s.config = Some(config);
        s.backend = Some(backend);
    });
    if initialize(
        Some(args.as_main_args()),
        Some(&settings),
        Some(&mut app),
        std::ptr::null_mut(),
    ) != 1
    {
        eprintln!(
            "[host] CEF failed to initialise (is another copy running on the same data folder?)"
        );
        return 1;
    }
    run_message_loop();
    shutdown();
    // Nothing holds the profile folders now. A safety net: anything a crash or a locked file left behind is removed here.
    // Chromium's child processes can take a moment to let go after shutdown, so retry for a few seconds before giving up (the next
    // start sweeps again).
    if let Some(backend) = state::with(|s| s.backend.take()) {
        for attempt in 0..20 {
            match backend.sweep_orphans() {
                Ok(removed) => {
                    for id in removed {
                        println!("[host] workspace {id}: data removed at exit");
                    }
                    break;
                }
                Err(e) if attempt < 19 => {
                    let _ = e;
                    std::thread::sleep(std::time::Duration::from_millis(250));
                }
                Err(e) => eprintln!(
                    "[host] could not remove deleted workspaces' data at exit ({e}); the next start will"
                ),
            }
        }
    }
    0
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_set() {
        assert!(!super::VERSION.is_empty());
    }
}
