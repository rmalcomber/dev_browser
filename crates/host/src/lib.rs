//! CEF integration for The Dev Browser: the only crate that touches CEF. All browser logic belongs in `dev_browser_core`; this crate
//! executes what core decides. One UI thread, handlers split by concern (see planning/m1-plan.md, section 4).

mod app;
mod handlers;
mod ipc;
mod layout;
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
    state::with(|s| s.config = Some(config));
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
    0
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_set() {
        assert!(!super::VERSION.is_empty());
    }
}
