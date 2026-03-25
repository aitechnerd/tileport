//! Manager thread: central coordinator for the tileport window manager.
//!
//! Receives commands from hotkey thread, AX events, and (future) IPC,
//! processes them through the WorkspaceManager, and applies window
//! transitions via the PlatformApi.

use crate::ipc::{IpcMessage, IpcResponse};
use crossbeam_channel::Receiver;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tileport_core::command::Command;
use tileport_core::config::Config;
use tileport_core::platform::{PlatformApi, WindowInfo};
use tileport_core::types::{Rect, WindowId};
use tileport_core::workspace::{WorkspaceManager, WorkspaceTransition};

/// Events from AX observers (window creation/destruction).
///
/// Currently constructed in tests and reserved for future AXObserver callbacks.
/// Window polling is handled directly in the manager loop via `poll_windows()`.
#[derive(Debug)]
#[allow(dead_code)]
pub enum AxEvent {
    WindowCreated {
        id: WindowId,
        app_id: String,
        title: String,
        /// AXWindow handle for registration with the platform layer.
        /// None in tests; Some when coming from the polling thread.
        ax_window: Option<tileport_macos::accessibility::AXWindow>,
    },
    WindowDestroyed {
        id: WindowId,
    },
}

/// Apply a WorkspaceTransition by moving windows via the platform API.
fn apply_transition(platform: &dyn PlatformApi, transition: &WorkspaceTransition) {
    for &(window_id, rect) in &transition.moves {
        if let Err(e) = platform.move_window(window_id, rect) {
            tracing::warn!(?window_id, error = %e, "failed to move window");
        }
    }
}

/// Process a command and apply the resulting transition.
///
/// Extracted for testability -- called by the manager loop.
pub fn process_command(
    command: &Command,
    workspace_mgr: &mut WorkspaceManager,
    platform: &dyn PlatformApi,
) {
    match command {
        Command::FocusNext => {
            let focused_before = workspace_mgr.active_workspace().layout.focused();
            let ws = workspace_mgr.active_workspace_mut();
            let new_focused = ws.layout.focus_next();

            if new_focused != focused_before {
                let transition = workspace_mgr.recalculate_active();
                apply_transition(platform, &transition);

                if let Some(id) = new_focused {
                    if let Err(e) = platform.focus_window(id) {
                        tracing::warn!(?id, error = %e, "failed to focus window");
                    }
                }
            }
        }
        Command::FocusPrev => {
            let focused_before = workspace_mgr.active_workspace().layout.focused();
            let ws = workspace_mgr.active_workspace_mut();
            let new_focused = ws.layout.focus_prev();

            if new_focused != focused_before {
                let transition = workspace_mgr.recalculate_active();
                apply_transition(platform, &transition);

                if let Some(id) = new_focused {
                    if let Err(e) = platform.focus_window(id) {
                        tracing::warn!(?id, error = %e, "failed to focus window");
                    }
                }
            }
        }
        Command::SwitchWorkspace { workspace } => {
            let transition = workspace_mgr.switch_workspace(*workspace);
            apply_transition(platform, &transition);

            if let Some(id) = workspace_mgr.active_workspace().layout.focused() {
                if let Err(e) = platform.focus_window(id) {
                    tracing::warn!(?id, error = %e, "failed to focus window");
                }
            }
        }
        Command::MoveToWorkspace { workspace } => {
            let transition = workspace_mgr.move_window_to_workspace(*workspace);
            apply_transition(platform, &transition);

            if let Some(id) = workspace_mgr.active_workspace().layout.focused() {
                if let Err(e) = platform.focus_window(id) {
                    tracing::warn!(?id, error = %e, "failed to focus window");
                }
            }
        }
        Command::ToggleFloat => {
            if let Some(id) = workspace_mgr.active_workspace().layout.focused() {
                let transition = workspace_mgr.toggle_float(id);
                apply_transition(platform, &transition);
            }
        }
        Command::ToggleFullscreen => {
            if let Some(id) = workspace_mgr.active_workspace().layout.focused() {
                let transition = workspace_mgr.toggle_fullscreen(id);
                apply_transition(platform, &transition);
            }
        }
        Command::FocusDirection { direction } => {
            let focused_before = workspace_mgr.active_workspace().layout.focused();
            let ws = workspace_mgr.active_workspace_mut();
            let new_focused = ws.layout.focus_direction(*direction);

            if new_focused != focused_before {
                let transition = workspace_mgr.recalculate_active();
                apply_transition(platform, &transition);

                if let Some(id) = new_focused {
                    if let Err(e) = platform.focus_window(id) {
                        tracing::warn!(?id, error = %e, "failed to focus window");
                    }
                }
            }
        }
        Command::MoveToZone { direction } => {
            let moved = workspace_mgr.move_to_zone(*direction);
            if moved {
                let transition = workspace_mgr.recalculate_active();
                apply_transition(platform, &transition);

                if let Some(id) = workspace_mgr.active_workspace().layout.focused() {
                    if let Err(e) = platform.focus_window(id) {
                        tracing::warn!(?id, error = %e, "failed to focus window");
                    }
                }
            }
        }
        Command::PromoteToPrimary => {
            let promoted = workspace_mgr.promote_to_primary();
            if promoted {
                let transition = workspace_mgr.recalculate_active();
                apply_transition(platform, &transition);

                if let Some(id) = workspace_mgr.active_workspace().layout.focused() {
                    if let Err(e) = platform.focus_window(id) {
                        tracing::warn!(?id, error = %e, "failed to focus window");
                    }
                }
            }
        }
        Command::Quit => {
            // Handled by the caller in manager_loop.
        }
    }
}

/// Process an AX event (window created/destroyed).
///
/// Extracted for testability -- called by the manager loop.
pub fn process_ax_event(
    event: &AxEvent,
    workspace_mgr: &mut WorkspaceManager,
    platform: &dyn PlatformApi,
) {
    match event {
        AxEvent::WindowCreated { id, app_id, title, .. } => {
            tracing::info!(?id, app_id, title, "window created");
            workspace_mgr.add_window(*id);
            let transition = workspace_mgr.recalculate_active();
            apply_transition(platform, &transition);

            if let Err(e) = platform.focus_window(*id) {
                tracing::warn!(?id, error = %e, "failed to focus new window");
            }
        }
        AxEvent::WindowDestroyed { id } => {
            tracing::info!(?id, "window destroyed");
            workspace_mgr.remove_window(*id);
            let transition = workspace_mgr.recalculate_active();
            apply_transition(platform, &transition);

            if let Some(focused_id) = workspace_mgr.active_workspace().layout.focused() {
                if let Err(e) = platform.focus_window(focused_id) {
                    tracing::warn!(?focused_id, error = %e, "failed to focus after destroy");
                }
            }
        }
    }
}

/// Perform graceful shutdown: restore all windows to visible positions.
pub fn shutdown(workspace_mgr: &WorkspaceManager, platform: &dyn PlatformApi) {
    tracing::info!("shutting down: restoring all windows to visible positions");
    let positions = workspace_mgr.get_all_window_positions();
    for &(window_id, rect) in &positions {
        if let Err(e) = platform.move_window(window_id, rect) {
            tracing::warn!(?window_id, error = %e, "failed to restore window on shutdown");
        }
    }
}

/// Trait for registering/unregistering AX window handles with the platform.
///
/// Implemented by `MacOSPlatform` in production. The mock platform in tests
/// uses the default no-op implementation.
pub trait WindowRegistry {
    fn register_ax_window(
        &mut self,
        _window_id: WindowId,
        _ax_window: tileport_macos::accessibility::AXWindow,
    ) {
    }
    fn unregister_ax_window(&mut self, _window_id: WindowId) {}

    /// Enumerate windows and register their AX handles in one step.
    ///
    /// This combines enumeration with handle registration so that polling
    /// can happen on the same thread that owns the platform state.
    /// Returns only `WindowInfo` since handles are stored internally.
    fn enumerate_and_register(&mut self) -> anyhow::Result<Vec<WindowInfo>> {
        Ok(Vec::new())
    }
}

impl WindowRegistry for tileport_macos::MacOSPlatform {
    fn register_ax_window(
        &mut self,
        window_id: WindowId,
        ax_window: tileport_macos::accessibility::AXWindow,
    ) {
        self.register_window(window_id, ax_window);
    }

    fn unregister_ax_window(&mut self, window_id: WindowId) {
        self.unregister_window(window_id);
    }

    fn enumerate_and_register(&mut self) -> anyhow::Result<Vec<WindowInfo>> {
        let results = tileport_macos::window::enumerate_windows()?;
        let mut infos = Vec::with_capacity(results.len());
        for (info, ax_window) in results {
            self.register_window(info.window_id, ax_window);
            infos.push(info);
        }
        Ok(infos)
    }
}

/// Poll for window changes: enumerate current windows, diff against known set,
/// and process creates/destroys.
///
/// This runs on the manager thread, which is safe for AX API calls because the
/// manager thread owns the platform state. Previously this logic lived in a
/// separate polling thread, which violated macOS AX API thread-safety requirements.
fn poll_windows<P: PlatformApi + WindowRegistry>(
    workspace_mgr: &mut WorkspaceManager,
    platform: &mut P,
    known_ids: &mut HashSet<WindowId>,
) {
    let current = match platform.enumerate_and_register() {
        Ok(infos) => infos,
        Err(e) => {
            tracing::warn!(error = %e, "poll: enumerate_windows failed");
            return;
        }
    };

    let current_ids: HashSet<WindowId> = current.iter().map(|info| info.window_id).collect();

    // Detect new windows (in current but not in known).
    for info in &current {
        if !known_ids.contains(&info.window_id) {
            tracing::info!(id = ?info.window_id, app = %info.app_id, "poll: new window detected");
            workspace_mgr.add_window(info.window_id);
            let transition = workspace_mgr.recalculate_active();
            apply_transition(platform, &transition);

            if let Err(e) = platform.focus_window(info.window_id) {
                tracing::warn!(id = ?info.window_id, error = %e, "failed to focus new window");
            }
        }
    }

    // Detect destroyed windows (in known but not in current).
    let destroyed: Vec<WindowId> = known_ids
        .iter()
        .copied()
        .filter(|id| !current_ids.contains(id))
        .collect();
    for id in &destroyed {
        tracing::info!(?id, "poll: window destroyed");
        platform.unregister_ax_window(*id);
        workspace_mgr.remove_window(*id);
        let transition = workspace_mgr.recalculate_active();
        apply_transition(platform, &transition);

        if let Some(focused_id) = workspace_mgr.active_workspace().layout.focused() {
            if let Err(e) = platform.focus_window(focused_id) {
                tracing::warn!(?focused_id, error = %e, "failed to focus after destroy");
            }
        }
    }

    *known_ids = current_ids;
}

/// Run the manager loop until shutdown.
///
/// This is the main function for the manager thread. It receives commands
/// from the hotkey thread and AX events, processes them, and applies
/// window transitions.
#[allow(clippy::too_many_arguments)]
pub fn manager_loop<P: PlatformApi + WindowRegistry>(
    hotkey_rx: Receiver<Command>,
    ax_rx: Receiver<AxEvent>,
    ipc_rx: Receiver<IpcMessage>,
    platform: &mut P,
    config: &Config,
    screen: Rect,
    initial_windows: Vec<WindowInfo>,
    shutdown_flag: Arc<AtomicBool>,
) {
    let layouts = config.build_workspace_layouts();
    let mut workspace_mgr = if layouts.is_empty() {
        WorkspaceManager::new()
    } else {
        WorkspaceManager::new_with_layouts(layouts)
    };
    workspace_mgr.set_screen_and_gaps(screen, config.gaps);

    // Add all initial windows to workspace 1.
    for win_info in &initial_windows {
        workspace_mgr.add_window(win_info.window_id);
    }

    // Apply initial layout.
    let transition = workspace_mgr.recalculate_active();
    apply_transition(platform, &transition);

    // Focus the focused window.
    if let Some(id) = workspace_mgr.active_workspace().layout.focused() {
        if let Err(e) = platform.focus_window(id) {
            tracing::warn!(?id, error = %e, "failed to focus initial window");
        }
    }

    // Build the initial set of known window IDs for polling diff.
    let mut known_ids: HashSet<WindowId> = initial_windows.iter().map(|w| w.window_id).collect();

    // Periodic timer for window polling (replaces the dedicated polling thread).
    // Polling on the manager thread is required because macOS AX API calls must
    // happen on the thread that owns the AX context.
    let poll_ticker = crossbeam_channel::tick(Duration::from_secs(2));

    // Periodic timer for shutdown flag checks (replaces the old default arm).
    let shutdown_ticker = crossbeam_channel::tick(Duration::from_millis(250));

    tracing::info!(
        window_count = initial_windows.len(),
        "manager loop starting"
    );

    loop {
        // Check shutdown flag before blocking on select.
        if shutdown_flag.load(Ordering::Relaxed) {
            tracing::info!("shutdown signal received");
            shutdown(&workspace_mgr, platform);
            return;
        }

        crossbeam_channel::select! {
            recv(hotkey_rx) -> msg => {
                match msg {
                    Ok(command) => {
                        tracing::debug!(?command, "received hotkey command");
                        if command == Command::Quit {
                            shutdown(&workspace_mgr, platform);
                            return;
                        }
                        process_command(&command, &mut workspace_mgr, platform);
                    }
                    Err(_) => {
                        tracing::info!("hotkey channel closed, shutting down");
                        shutdown(&workspace_mgr, platform);
                        return;
                    }
                }
            }
            recv(ax_rx) -> msg => {
                match msg {
                    Ok(event) => {
                        tracing::debug!(?event, "received AX event");
                        // Register/unregister AX window handles with the platform layer
                        // before processing the event (so move/focus calls can find the handle).
                        match &event {
                            AxEvent::WindowCreated { id, ax_window, .. } => {
                                if let Some(ax_win) = ax_window {
                                    platform.register_ax_window(*id, ax_win.clone());
                                }
                            }
                            AxEvent::WindowDestroyed { id } => {
                                platform.unregister_ax_window(*id);
                            }
                        }
                        process_ax_event(&event, &mut workspace_mgr, platform);
                    }
                    Err(_) => {
                        tracing::info!("AX event channel closed");
                    }
                }
            }
            recv(ipc_rx) -> msg => {
                match msg {
                    Ok((command, resp_tx)) => {
                        tracing::debug!(?command, "received IPC command");
                        if command == Command::Quit {
                            // Send OK response before shutting down.
                            let _ = resp_tx.send(IpcResponse::ok());
                            shutdown(&workspace_mgr, platform);
                            return;
                        }
                        process_command(&command, &mut workspace_mgr, platform);
                        // Send response back -- don't block if receiver is gone.
                        let _ = resp_tx.send(IpcResponse::ok());
                    }
                    Err(_) => {
                        tracing::info!("IPC channel closed");
                    }
                }
            }
            recv(poll_ticker) -> _ => {
                poll_windows(&mut workspace_mgr, platform, &mut known_ids);
            }
            recv(shutdown_ticker) -> _ => {
                if shutdown_flag.load(Ordering::Relaxed) {
                    tracing::info!("shutdown signal received (periodic check)");
                    shutdown(&workspace_mgr, platform);
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Mock platform that records all calls for test assertions.
    struct MockPlatform {
        move_calls: Mutex<Vec<(WindowId, Rect)>>,
        focus_calls: Mutex<Vec<WindowId>>,
        /// Windows returned by `enumerate_and_register` (for poll testing).
        poll_windows: Mutex<Vec<WindowInfo>>,
    }

    impl MockPlatform {
        fn new() -> Self {
            Self {
                move_calls: Mutex::new(Vec::new()),
                focus_calls: Mutex::new(Vec::new()),
                poll_windows: Mutex::new(Vec::new()),
            }
        }

        fn move_calls(&self) -> Vec<(WindowId, Rect)> {
            self.move_calls.lock().unwrap().clone()
        }

        fn focus_calls(&self) -> Vec<WindowId> {
            self.focus_calls.lock().unwrap().clone()
        }

        fn clear(&self) {
            self.move_calls.lock().unwrap().clear();
            self.focus_calls.lock().unwrap().clear();
        }

        /// Set the windows that will be returned by the next `enumerate_and_register` call.
        fn set_poll_windows(&self, windows: Vec<WindowInfo>) {
            *self.poll_windows.lock().unwrap() = windows;
        }
    }

    impl WindowRegistry for MockPlatform {
        fn enumerate_and_register(&mut self) -> anyhow::Result<Vec<WindowInfo>> {
            Ok(self.poll_windows.lock().unwrap().clone())
        }
    }

    impl PlatformApi for MockPlatform {
        fn enumerate_windows(&self) -> anyhow::Result<Vec<WindowInfo>> {
            Ok(Vec::new())
        }

        fn move_window(&self, window_id: WindowId, rect: Rect) -> anyhow::Result<()> {
            self.move_calls.lock().unwrap().push((window_id, rect));
            Ok(())
        }

        fn get_display_bounds(&self) -> anyhow::Result<Rect> {
            Ok(Rect {
                x: 0.0,
                y: 0.0,
                width: 1920.0,
                height: 1080.0,
            })
        }

        fn focus_window(&self, window_id: WindowId) -> anyhow::Result<()> {
            self.focus_calls.lock().unwrap().push(window_id);
            Ok(())
        }
    }

    fn wid(n: u32) -> WindowId {
        WindowId(n)
    }

    fn screen() -> Rect {
        Rect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        }
    }

    fn test_config() -> Config {
        Config::default()
    }

    fn setup_manager_with_windows(window_ids: &[u32]) -> WorkspaceManager {
        let mut mgr = WorkspaceManager::new();
        mgr.set_screen_and_gaps(
            screen(),
            tileport_core::types::Gaps {
                inner: 8.0,
                outer: 10.0,
            },
        );
        for &id in window_ids {
            mgr.add_window(wid(id));
        }
        mgr
    }

    #[test]
    fn test_process_focus_next() {
        let platform = MockPlatform::new();
        let mut mgr = setup_manager_with_windows(&[1, 2, 3]);
        // focused = wid(3) after adding all three

        process_command(&Command::FocusNext, &mut mgr, &platform);

        // Focus should wrap to wid(1).
        assert_eq!(mgr.active_workspace().layout.focused(), Some(wid(1)));

        // Platform should have received move_window calls for repositioning.
        let moves = platform.move_calls();
        assert!(!moves.is_empty(), "should have move calls for layout");

        // The focused window (wid(1)) should be on-screen.
        let focused_move = moves.iter().find(|(id, _)| *id == wid(1));
        assert!(focused_move.is_some(), "focused window should be moved");
        let (_, rect) = focused_move.unwrap();
        assert!(rect.x < 10000.0, "focused window should be on-screen");

        // Other windows should be offscreen.
        for (id, rect) in &moves {
            if *id != wid(1) {
                assert_eq!(rect.x, 10000.0, "non-focused window should be offscreen");
            }
        }

        // Should have called focus_window on the new focused window.
        assert!(platform.focus_calls().contains(&wid(1)));
    }

    #[test]
    fn test_process_focus_prev() {
        let platform = MockPlatform::new();
        let mut mgr = setup_manager_with_windows(&[1, 2, 3]);
        // focused = wid(3)

        process_command(&Command::FocusPrev, &mut mgr, &platform);

        assert_eq!(mgr.active_workspace().layout.focused(), Some(wid(2)));
        assert!(platform.focus_calls().contains(&wid(2)));
    }

    #[test]
    fn test_process_switch_workspace() {
        let platform = MockPlatform::new();
        let mut mgr = setup_manager_with_windows(&[1, 2]);

        // Switch to workspace 3 (empty).
        process_command(
            &Command::SwitchWorkspace { workspace: 3 },
            &mut mgr,
            &platform,
        );

        assert_eq!(mgr.active_index(), 2);

        // Windows from workspace 1 should be moved offscreen.
        let moves = platform.move_calls();
        let offscreen_moves: Vec<_> = moves
            .iter()
            .filter(|(_, rect)| rect.x >= 10000.0)
            .collect();
        assert_eq!(offscreen_moves.len(), 2, "both windows should go offscreen");
    }

    #[test]
    fn test_process_move_to_workspace() {
        let platform = MockPlatform::new();
        let mut mgr = setup_manager_with_windows(&[1, 2]);
        // focused = wid(2)

        process_command(
            &Command::MoveToWorkspace { workspace: 3 },
            &mut mgr,
            &platform,
        );

        // wid(2) should be in workspace 3.
        assert!(mgr.workspace(3).layout.windows().contains(&wid(2)));
        // wid(1) should still be in workspace 1.
        assert!(mgr.workspace(1).layout.windows().contains(&wid(1)));
        // Active workspace is still 1.
        assert_eq!(mgr.active_index(), 0);

        // Platform should have moved wid(2) offscreen.
        let moves = platform.move_calls();
        let wid2_move = moves.iter().find(|(id, _)| *id == wid(2)).unwrap();
        assert_eq!(wid2_move.1.x, 10000.0);

        // wid(1) should now be visible (it's the new focused window on ws1).
        let wid1_move = moves.iter().find(|(id, _)| *id == wid(1)).unwrap();
        assert!(wid1_move.1.x < 10000.0);
    }

    #[test]
    fn test_shutdown_restores_all_windows() {
        let platform = MockPlatform::new();
        let mut mgr = setup_manager_with_windows(&[1, 2, 3]);

        // Move wid(3) to workspace 2 (it goes offscreen).
        mgr.move_window_to_workspace(2);

        platform.clear();
        shutdown(&mgr, &platform);

        let moves = platform.move_calls();
        // All 3 windows should be restored to visible positions.
        assert_eq!(moves.len(), 3);
        for (_, rect) in &moves {
            assert!(rect.x < 10000.0, "all windows should be on-screen after shutdown");
        }
    }

    #[test]
    fn test_window_created_event() {
        let platform = MockPlatform::new();
        let mut mgr = setup_manager_with_windows(&[1]);
        platform.clear();

        process_ax_event(
            &AxEvent::WindowCreated {
                id: wid(2),
                app_id: "com.test.app".into(),
                title: "New Window".into(),
                ax_window: None,
            },
            &mut mgr,
            &platform,
        );

        // Window 2 should be added and become focused.
        assert_eq!(mgr.active_workspace().layout.len(), 2);
        assert_eq!(mgr.active_workspace().layout.focused(), Some(wid(2)));

        // Platform should have moved windows.
        let moves = platform.move_calls();
        assert!(!moves.is_empty());

        // wid(2) should be on-screen (focused).
        let wid2_move = moves.iter().find(|(id, _)| *id == wid(2));
        assert!(wid2_move.is_some());
        assert!(wid2_move.unwrap().1.x < 10000.0);

        // Should have focused the new window.
        assert!(platform.focus_calls().contains(&wid(2)));
    }

    #[test]
    fn test_window_destroyed_event() {
        let platform = MockPlatform::new();
        let mut mgr = setup_manager_with_windows(&[1, 2, 3]);
        // focused = wid(3)
        platform.clear();

        process_ax_event(
            &AxEvent::WindowDestroyed { id: wid(3) },
            &mut mgr,
            &platform,
        );

        // Window 3 should be removed.
        assert_eq!(mgr.active_workspace().layout.len(), 2);
        // Focus should promote to wid(2).
        assert_eq!(mgr.active_workspace().layout.focused(), Some(wid(2)));

        // Should have focused the promoted window.
        assert!(platform.focus_calls().contains(&wid(2)));
    }

    #[test]
    fn test_manager_loop_quit_command() {
        let mut platform = MockPlatform::new();
        let config = test_config();
        let (hotkey_tx, hotkey_rx) = crossbeam_channel::bounded(16);
        let (_ax_tx, ax_rx) = crossbeam_channel::bounded::<AxEvent>(16);
        let (_ipc_tx, ipc_rx) = crossbeam_channel::bounded::<IpcMessage>(16);
        let shutdown_flag = Arc::new(AtomicBool::new(false));

        let initial_windows = vec![WindowInfo {
            window_id: wid(1),
            app_id: "com.test".into(),
            title: "Test".into(),
            pid: 1,
        }];

        // Send quit command.
        hotkey_tx.send(Command::Quit).unwrap();

        manager_loop(
            hotkey_rx,
            ax_rx,
            ipc_rx,
            &mut platform,
            &config,
            screen(),
            initial_windows,
            shutdown_flag,
        );

        // Shutdown should have restored the window.
        let moves = platform.move_calls();
        assert!(!moves.is_empty());
    }

    #[test]
    fn test_manager_loop_shutdown_flag() {
        let mut platform = MockPlatform::new();
        let config = test_config();
        let (_hotkey_tx, hotkey_rx) = crossbeam_channel::bounded(16);
        let (_ax_tx, ax_rx) = crossbeam_channel::bounded::<AxEvent>(16);
        let (_ipc_tx, ipc_rx) = crossbeam_channel::bounded::<IpcMessage>(16);
        let shutdown_flag = Arc::new(AtomicBool::new(true)); // Already set.

        manager_loop(
            hotkey_rx,
            ax_rx,
            ipc_rx,
            &mut platform,
            &config,
            screen(),
            vec![],
            shutdown_flag,
        );

        // Should exit immediately due to shutdown flag.
    }

    #[test]
    fn test_manager_loop_ipc_quit() {
        let mut platform = MockPlatform::new();
        let config = test_config();
        let (_hotkey_tx, hotkey_rx) = crossbeam_channel::bounded(16);
        let (_ax_tx, ax_rx) = crossbeam_channel::bounded::<AxEvent>(16);
        let (ipc_tx, ipc_rx) = crossbeam_channel::bounded::<IpcMessage>(16);
        let shutdown_flag = Arc::new(AtomicBool::new(false));

        // Send quit command via IPC.
        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
        ipc_tx.send((Command::Quit, resp_tx)).unwrap();

        manager_loop(
            hotkey_rx,
            ax_rx,
            ipc_rx,
            &mut platform,
            &config,
            screen(),
            vec![],
            shutdown_flag,
        );

        // Response should be OK.
        let resp = resp_rx.blocking_recv().unwrap();
        assert_eq!(resp, IpcResponse::ok());
    }

    #[test]
    fn test_manager_loop_ipc_command() {
        let mut platform = MockPlatform::new();
        let config = test_config();
        let (_hotkey_tx, hotkey_rx) = crossbeam_channel::bounded(16);
        let (_ax_tx, ax_rx) = crossbeam_channel::bounded::<AxEvent>(16);
        let (ipc_tx, ipc_rx) = crossbeam_channel::bounded::<IpcMessage>(16);
        let shutdown_flag = Arc::new(AtomicBool::new(false));

        let initial_windows = vec![
            WindowInfo {
                window_id: wid(1),
                app_id: "com.test".into(),
                title: "Test 1".into(),
                pid: 1,
            },
            WindowInfo {
                window_id: wid(2),
                app_id: "com.test".into(),
                title: "Test 2".into(),
                pid: 1,
            },
        ];

        // Send focus_next via IPC, then quit via IPC.
        let (resp_tx1, resp_rx1) = tokio::sync::oneshot::channel();
        ipc_tx.send((Command::FocusNext, resp_tx1)).unwrap();
        let (resp_tx2, _resp_rx2) = tokio::sync::oneshot::channel();
        ipc_tx.send((Command::Quit, resp_tx2)).unwrap();

        manager_loop(
            hotkey_rx,
            ax_rx,
            ipc_rx,
            &mut platform,
            &config,
            screen(),
            initial_windows,
            shutdown_flag,
        );

        // First response should be OK.
        let resp = resp_rx1.blocking_recv().unwrap();
        assert_eq!(resp, IpcResponse::ok());
    }

    #[test]
    fn test_toggle_fullscreen_command() {
        let platform = MockPlatform::new();
        let mut mgr = setup_manager_with_windows(&[1]);
        platform.clear();

        process_command(&Command::ToggleFullscreen, &mut mgr, &platform);

        assert!(mgr.active_workspace().is_fullscreen(wid(1)));

        let moves = platform.move_calls();
        let (_, rect) = moves.iter().find(|(id, _)| *id == wid(1)).unwrap();
        assert_eq!(rect.x, 0.0);
        assert_eq!(rect.y, 0.0);
        assert_eq!(rect.width, 1920.0);
        assert_eq!(rect.height, 1080.0);
    }

    #[test]
    fn test_toggle_float_command() {
        let platform = MockPlatform::new();
        let mut mgr = setup_manager_with_windows(&[1, 2]);
        // focused = wid(2)
        platform.clear();

        process_command(&Command::ToggleFloat, &mut mgr, &platform);

        let ws = mgr.active_workspace();
        assert!(ws.floating_windows.contains(&wid(2)));
        assert!(!ws.layout.windows().contains(&wid(2)));
    }

    /// Test that the manager loop processes AX events from the ax_rx channel.
    ///
    /// Sends a WindowCreated event via ax_rx first, waits for it to be processed,
    /// then sends Quit. This validates that the ax_rx channel (reserved for future
    /// AXObserver callbacks) is still functional in the select loop.
    #[test]
    fn test_manager_loop_ax_events() {
        let mut platform = MockPlatform::new();
        let config = test_config();
        let (_hotkey_tx, hotkey_rx) = crossbeam_channel::bounded(16);
        let (ax_tx, ax_rx) = crossbeam_channel::bounded::<AxEvent>(16);
        let (ipc_tx, ipc_rx) = crossbeam_channel::bounded::<IpcMessage>(16);
        let shutdown_flag = Arc::new(AtomicBool::new(false));

        let initial_windows = vec![WindowInfo {
            window_id: wid(1),
            app_id: "com.test".into(),
            title: "Test".into(),
            pid: 1,
        }];

        // Send window created event via ax_rx.
        ax_tx
            .send(AxEvent::WindowCreated {
                id: wid(2),
                app_id: "com.test.new".into(),
                title: "New Window".into(),
                ax_window: None,
            })
            .unwrap();

        // Run manager_loop in a background thread so we can control timing.
        let sf = Arc::clone(&shutdown_flag);
        let handle = std::thread::spawn(move || {
            manager_loop(
                hotkey_rx,
                ax_rx,
                ipc_rx,
                &mut platform,
                &config,
                screen(),
                initial_windows,
                sf,
            );
            platform
        });

        // Give the manager loop time to process the ax_event before sending quit.
        std::thread::sleep(Duration::from_millis(100));

        let (resp_tx, _resp_rx) = tokio::sync::oneshot::channel();
        ipc_tx.send((Command::Quit, resp_tx)).unwrap();

        let platform = handle.join().unwrap();

        // Shutdown should have restored both windows (wid(1) and wid(2)).
        let moves = platform.move_calls();
        let restored_ids: Vec<WindowId> = moves.iter().map(|(id, _)| *id).collect();
        assert!(
            restored_ids.contains(&wid(1)),
            "original window should be restored"
        );
        assert!(
            restored_ids.contains(&wid(2)),
            "ax-event window should be restored"
        );
    }

    #[test]
    fn test_poll_windows_detects_new_window() {
        let mut platform = MockPlatform::new();
        let mut mgr = setup_manager_with_windows(&[1]);
        let mut known_ids: HashSet<WindowId> = [wid(1)].into_iter().collect();

        // Simulate a new window appearing.
        platform.set_poll_windows(vec![
            WindowInfo {
                window_id: wid(1),
                app_id: "com.test".into(),
                title: "Existing".into(),
                pid: 1,
            },
            WindowInfo {
                window_id: wid(2),
                app_id: "com.test.new".into(),
                title: "New Window".into(),
                pid: 2,
            },
        ]);

        poll_windows(&mut mgr, &mut platform, &mut known_ids);

        // wid(2) should have been added to the workspace.
        assert_eq!(mgr.active_workspace().layout.len(), 2);
        assert!(known_ids.contains(&wid(2)));
        // Should have focused the new window.
        assert!(platform.focus_calls().contains(&wid(2)));
    }

    #[test]
    fn test_poll_windows_detects_destroyed_window() {
        let mut platform = MockPlatform::new();
        let mut mgr = setup_manager_with_windows(&[1, 2]);
        let mut known_ids: HashSet<WindowId> = [wid(1), wid(2)].into_iter().collect();
        platform.clear();

        // Simulate wid(2) being destroyed.
        platform.set_poll_windows(vec![WindowInfo {
            window_id: wid(1),
            app_id: "com.test".into(),
            title: "Remaining".into(),
            pid: 1,
        }]);

        poll_windows(&mut mgr, &mut platform, &mut known_ids);

        // wid(2) should have been removed.
        assert_eq!(mgr.active_workspace().layout.len(), 1);
        assert!(!known_ids.contains(&wid(2)));
        assert!(known_ids.contains(&wid(1)));
    }

    #[test]
    fn test_poll_windows_no_changes() {
        let mut platform = MockPlatform::new();
        let mut mgr = setup_manager_with_windows(&[1]);
        let mut known_ids: HashSet<WindowId> = [wid(1)].into_iter().collect();
        platform.clear();

        // Same window set, no changes.
        platform.set_poll_windows(vec![WindowInfo {
            window_id: wid(1),
            app_id: "com.test".into(),
            title: "Same".into(),
            pid: 1,
        }]);

        poll_windows(&mut mgr, &mut platform, &mut known_ids);

        // No new moves or focus calls (only from initial setup which we cleared).
        assert!(platform.move_calls().is_empty());
        assert!(platform.focus_calls().is_empty());
        assert_eq!(mgr.active_workspace().layout.len(), 1);
    }

    // --- Zone command tests ---

    use tileport_core::workspace::WorkspaceLayout;
    use tileport_core::zone::{Direction, ZoneLayout, ZoneNode};

    /// Create a 2-column (50/50) zone workspace manager with windows added.
    fn setup_zone_manager_with_windows(window_ids: &[u32]) -> WorkspaceManager {
        let root = ZoneNode::HSplit {
            ratios: vec![0.5, 0.5],
            children: vec![ZoneNode::Leaf, ZoneNode::Leaf],
        };
        let zone_layout = ZoneLayout::new(root, vec![0, 1], 0);
        let layout = WorkspaceLayout::Zone(zone_layout);

        let mut mgr = WorkspaceManager::new();
        mgr.set_screen_and_gaps(
            screen(),
            tileport_core::types::Gaps {
                inner: 8.0,
                outer: 10.0,
            },
        );

        // Replace workspace 1's layout with the zone layout.
        *mgr.workspace_mut(1) =
            tileport_core::workspace::Workspace::new_with_layout(1, layout);

        for &id in window_ids {
            mgr.add_window(wid(id));
        }
        mgr
    }

    #[test]
    fn test_process_focus_direction_zone() {
        let platform = MockPlatform::new();
        // 2 windows in a 2-column zone: wid(1) in zone 0, wid(2) in zone 1.
        let mut mgr = setup_zone_manager_with_windows(&[1, 2]);
        // After adding, focused is wid(2) in zone 1.
        assert_eq!(mgr.active_workspace().layout.focused(), Some(wid(2)));
        platform.clear();

        // FocusDirection Left should move focus to wid(1) in zone 0.
        process_command(
            &Command::FocusDirection {
                direction: Direction::Left,
            },
            &mut mgr,
            &platform,
        );

        assert_eq!(mgr.active_workspace().layout.focused(), Some(wid(1)));
        assert!(platform.focus_calls().contains(&wid(1)));
        // Should have recalculated layout (move calls).
        assert!(!platform.move_calls().is_empty());
    }

    #[test]
    fn test_process_focus_direction_monocle_fallback() {
        let platform = MockPlatform::new();
        // Monocle workspace (default).
        let mut mgr = setup_manager_with_windows(&[1, 2, 3]);
        // focused = wid(3)
        platform.clear();

        // FocusDirection Down should act as focus_next (wrap to wid(1)).
        process_command(
            &Command::FocusDirection {
                direction: Direction::Down,
            },
            &mut mgr,
            &platform,
        );

        assert_eq!(mgr.active_workspace().layout.focused(), Some(wid(1)));
        assert!(platform.focus_calls().contains(&wid(1)));
    }

    #[test]
    fn test_process_move_to_zone() {
        let platform = MockPlatform::new();
        // 2 windows in 2-column zone.
        let mut mgr = setup_zone_manager_with_windows(&[1, 2]);
        // wid(1) in zone 0, wid(2) in zone 1. Focused = wid(2).
        platform.clear();

        // Move wid(2) to the left zone (swap with wid(1)).
        process_command(
            &Command::MoveToZone {
                direction: Direction::Left,
            },
            &mut mgr,
            &platform,
        );

        // After move, focused window should still be wid(2).
        assert_eq!(mgr.active_workspace().layout.focused(), Some(wid(2)));
        // Layout should have been recalculated.
        assert!(!platform.move_calls().is_empty());
        // Focus should have been called.
        assert!(platform.focus_calls().contains(&wid(2)));
    }

    #[test]
    fn test_process_promote_to_primary() {
        let platform = MockPlatform::new();
        // 2 windows in 2-column zone. Primary zone = 0.
        let mut mgr = setup_zone_manager_with_windows(&[1, 2]);
        // wid(1) in zone 0 (primary), wid(2) in zone 1. Focused = wid(2).
        platform.clear();

        // Promote wid(2) to primary zone (swap with wid(1)).
        process_command(&Command::PromoteToPrimary, &mut mgr, &platform);

        // After promote, focused window should still be wid(2).
        assert_eq!(mgr.active_workspace().layout.focused(), Some(wid(2)));
        // Layout should have been recalculated.
        assert!(!platform.move_calls().is_empty());
        assert!(platform.focus_calls().contains(&wid(2)));
    }

    #[test]
    fn test_process_focus_direction_boundary_noop() {
        let platform = MockPlatform::new();
        // 2 windows in 2-column zone.
        let mut mgr = setup_zone_manager_with_windows(&[1, 2]);
        // Focus wid(1) in zone 0 first.
        process_command(
            &Command::FocusDirection {
                direction: Direction::Left,
            },
            &mut mgr,
            &platform,
        );
        assert_eq!(mgr.active_workspace().layout.focused(), Some(wid(1)));
        platform.clear();

        // FocusDirection Left again at boundary -- should be a no-op.
        process_command(
            &Command::FocusDirection {
                direction: Direction::Left,
            },
            &mut mgr,
            &platform,
        );

        // Focus unchanged.
        assert_eq!(mgr.active_workspace().layout.focused(), Some(wid(1)));
        // No platform calls since focus didn't change.
        assert!(platform.move_calls().is_empty());
        assert!(platform.focus_calls().is_empty());
    }
}
