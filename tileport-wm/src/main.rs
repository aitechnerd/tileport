//! tileport-wm: daemon entry point.
//!
//! Startup sequence:
//! 1. Init tracing
//! 2. Load config (hardcoded defaults for Phase 4; finalized in Phase 5)
//! 3. Check permissions
//! 4. Query display bounds
//! 5. Enumerate existing windows
//! 6. Create channels
//! 7. Set up signal handler (SIGINT/SIGTERM -> AtomicBool)
//! 8. Spawn manager thread
//! 9. Spawn hotkey thread
//! 10. Run NSApplication on main thread

mod ipc;
mod manager;

use anyhow::Result;
use crossbeam_channel::bounded;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tileport_core::config::Config;
use tileport_macos::MacOSPlatform;

fn main() -> Result<()> {
    // 1. Initialize tracing with env filter (default INFO).
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("tileport-wm starting");

    // 2. Load config from file, with XDG fallback and hardcoded defaults.
    let config = load_config();
    tracing::info!(gaps = ?config.gaps, "config loaded");

    // 3. Check permissions -- exit with error if missing (AC-16).
    tileport_macos::permission::ensure_permissions()?;
    tracing::info!("permissions verified");

    // 4. Query display bounds.
    let platform = MacOSPlatform::new();
    let screen = tileport_macos::display::get_primary_display()?;
    tracing::info!(?screen, "display bounds");

    // 5. Enumerate existing windows.
    let enumerated = tileport_macos::window::enumerate_windows()?;
    tracing::info!(count = enumerated.len(), "enumerated windows");

    // Register AX windows with the platform for later move/focus calls.
    let mut platform = platform;
    let mut initial_windows = Vec::new();
    for (info, ax_window) in enumerated {
        tracing::info!(id = ?info.window_id, app = %info.app_id, "discovered window");
        tracing::debug!(id = ?info.window_id, title = %info.title, "window title");
        platform.register_window(info.window_id, ax_window);
        initial_windows.push(info);
    }

    // 6. Create crossbeam channels.
    //    hotkey_tx/rx: hotkey thread -> manager (bounded to prevent backpressure issues).
    //    ax_tx/rx: AX events -> manager (future: from AXObserver or polling).
    //    ipc_tx/rx: IPC thread -> manager (carries command + response channel).
    let (hotkey_tx, hotkey_rx) = bounded(64);
    let (_ax_tx, ax_rx) = bounded::<manager::AxEvent>(64);
    let (ipc_tx, ipc_rx) = bounded::<ipc::IpcMessage>(64);

    // 7. Set up signal handler: SIGINT/SIGTERM -> set AtomicBool flag.
    //    DevSecOps requirement: only set an atomic flag in the handler.
    //    No heap allocation, no AX calls, no locks.
    let shutdown_flag = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&shutdown_flag))?;
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&shutdown_flag))?;
    tracing::info!("signal handlers registered (SIGINT, SIGTERM)");

    // 8. Spawn manager thread.
    let manager_config = config.clone();
    let manager_shutdown = Arc::clone(&shutdown_flag);
    let manager_handle = std::thread::Builder::new()
        .name("tileport-manager".into())
        .spawn(move || {
            manager::manager_loop(
                hotkey_rx,
                ax_rx,
                ipc_rx,
                &platform,
                &manager_config,
                screen,
                initial_windows,
                manager_shutdown,
            );
        })?;

    // 8b. Spawn IPC thread (Unix socket server).
    let ipc_shutdown = Arc::clone(&shutdown_flag);
    let _ipc_handle = ipc::start_ipc_thread(ipc_tx, ipc_shutdown);
    tracing::info!("IPC thread spawned");

    // 9. Spawn hotkey thread.
    let _hotkey_handle =
        tileport_macos::hotkey::start_hotkey_thread(config.keybindings.clone(), hotkey_tx);

    tracing::info!("all threads spawned, starting NSApplication run loop");

    // 10. Run NSApplication on main thread.
    //     Required by macOS for AX observers and event processing.
    //     No menu bar, no tray icon -- just the run loop.
    run_nsapplication();

    // If NSApplication exits (shouldn't normally), wait for manager.
    let _ = manager_handle.join();

    tracing::info!("tileport-wm exiting");
    Ok(())
}

/// Load configuration with the following priority:
/// 1. `~/.config/tileport/tileport.toml` (explicit path)
/// 2. XDG config dir via `directories` crate (ProjectDirs)
/// 3. Hardcoded defaults (AC-15)
///
/// If the file exists but has parse errors, logs a warning and falls back to defaults.
fn load_config() -> Config {
    use directories::ProjectDirs;

    // Try ~/.config/tileport/tileport.toml first.
    let explicit_path = dirs_config_path();
    if let Some(path) = &explicit_path {
        if path.exists() {
            match Config::load_from_file(path) {
                Ok(config) => {
                    tracing::info!(?path, "loaded config from file");
                    return config;
                }
                Err(e) => {
                    tracing::warn!(?path, error = %e, "config file has errors, using defaults");
                    return Config::default();
                }
            }
        }
    }

    // Try XDG config dir.
    if let Some(proj_dirs) = ProjectDirs::from("", "", "tileport") {
        let xdg_path = proj_dirs.config_dir().join("tileport.toml");
        if xdg_path.exists() {
            match Config::load_from_file(&xdg_path) {
                Ok(config) => {
                    tracing::info!(?xdg_path, "loaded config from XDG path");
                    return config;
                }
                Err(e) => {
                    tracing::warn!(?xdg_path, error = %e, "XDG config file has errors, using defaults");
                    return Config::default();
                }
            }
        }
    }

    tracing::info!("no config file found, using defaults");
    Config::default()
}

/// Get the explicit config path: `~/.config/tileport/tileport.toml`.
fn dirs_config_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join(".config").join("tileport").join("tileport.toml"))
}

/// Return the home directory.
mod dirs {
    use std::path::PathBuf;

    pub fn home_dir() -> Option<PathBuf> {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

/// Start a minimal NSApplication run loop on the main thread.
///
/// macOS requires an NSApplication for AX observers and event processing.
/// This creates a shared application instance and runs it. The run loop
/// keeps the main thread alive and dispatches events.
fn run_nsapplication() {
    use objc2_app_kit::NSApplication;
    use objc2_foundation::MainThreadMarker;

    // SAFETY: We are on the main thread (this is called from main()).
    let mtm = MainThreadMarker::new().expect("must be called from the main thread");
    let app = NSApplication::sharedApplication(mtm);

    // Set activation policy to Accessory (no Dock icon, no menu bar).
    use objc2_app_kit::NSApplicationActivationPolicy;
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    // Run the application event loop. This blocks until the app terminates.
    app.run();
}
