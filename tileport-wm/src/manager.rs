//! Manager thread: central coordinator for the tileport window manager.
//!
//! Receives commands from hotkey thread, AX events, and (future) IPC,
//! processes them through the WorkspaceManager, and applies window
//! transitions via the PlatformApi.

use crate::ipc::{IpcMessage, IpcResponse};
use crossbeam_channel::Receiver;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tileport_core::command::Command;
use tileport_core::config::Config;
use tileport_core::platform::{PlatformApi, WindowInfo};
use tileport_core::types::{Rect, WindowId};
use tileport_core::workspace::{WorkspaceManager, WorkspaceTransition};

/// Events from the AX observer (window creation/destruction).
///
/// Variants are constructed by AXObserver callbacks or polling (not yet wired),
/// and in tests. Allow dead_code until Phase 5 / AXObserver integration.
#[derive(Debug)]
#[allow(dead_code)]
pub enum AxEvent {
    WindowCreated {
        id: WindowId,
        app_id: String,
        title: String,
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
            let focused_before = workspace_mgr.active_workspace().monocle.focused();
            let ws = workspace_mgr.active_workspace_mut();
            let new_focused = ws.monocle.focus_next();

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
            let focused_before = workspace_mgr.active_workspace().monocle.focused();
            let ws = workspace_mgr.active_workspace_mut();
            let new_focused = ws.monocle.focus_prev();

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

            if let Some(id) = workspace_mgr.active_workspace().monocle.focused() {
                if let Err(e) = platform.focus_window(id) {
                    tracing::warn!(?id, error = %e, "failed to focus window");
                }
            }
        }
        Command::MoveToWorkspace { workspace } => {
            let transition = workspace_mgr.move_window_to_workspace(*workspace);
            apply_transition(platform, &transition);

            if let Some(id) = workspace_mgr.active_workspace().monocle.focused() {
                if let Err(e) = platform.focus_window(id) {
                    tracing::warn!(?id, error = %e, "failed to focus window");
                }
            }
        }
        Command::ToggleFloat => {
            if let Some(id) = workspace_mgr.active_workspace().monocle.focused() {
                let transition = workspace_mgr.toggle_float(id);
                apply_transition(platform, &transition);
            }
        }
        Command::ToggleFullscreen => {
            if let Some(id) = workspace_mgr.active_workspace().monocle.focused() {
                let transition = workspace_mgr.toggle_fullscreen(id);
                apply_transition(platform, &transition);
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
        AxEvent::WindowCreated { id, app_id, title } => {
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

            if let Some(focused_id) = workspace_mgr.active_workspace().monocle.focused() {
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

/// Run the manager loop until shutdown.
///
/// This is the main function for the manager thread. It receives commands
/// from the hotkey thread and AX events, processes them, and applies
/// window transitions.
#[allow(clippy::too_many_arguments)]
pub fn manager_loop(
    hotkey_rx: Receiver<Command>,
    ax_rx: Receiver<AxEvent>,
    ipc_rx: Receiver<IpcMessage>,
    platform: &dyn PlatformApi,
    config: &Config,
    screen: Rect,
    initial_windows: Vec<WindowInfo>,
    shutdown_flag: Arc<AtomicBool>,
) {
    let mut workspace_mgr = WorkspaceManager::new();
    workspace_mgr.set_screen_and_gaps(screen, config.gaps);

    // Add all initial windows to workspace 1.
    for win_info in &initial_windows {
        workspace_mgr.add_window(win_info.window_id);
    }

    // Apply initial layout.
    let transition = workspace_mgr.recalculate_active();
    apply_transition(platform, &transition);

    // Focus the focused window.
    if let Some(id) = workspace_mgr.active_workspace().monocle.focused() {
        if let Err(e) = platform.focus_window(id) {
            tracing::warn!(?id, error = %e, "failed to focus initial window");
        }
    }

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
            // Periodic check for shutdown flag (every 250ms).
            default(Duration::from_millis(250)) => {
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
    }

    impl MockPlatform {
        fn new() -> Self {
            Self {
                move_calls: Mutex::new(Vec::new()),
                focus_calls: Mutex::new(Vec::new()),
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
        assert_eq!(mgr.active_workspace().monocle.focused(), Some(wid(1)));

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

        assert_eq!(mgr.active_workspace().monocle.focused(), Some(wid(2)));
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
        assert!(mgr.workspace(3).monocle.windows().contains(&wid(2)));
        // wid(1) should still be in workspace 1.
        assert!(mgr.workspace(1).monocle.windows().contains(&wid(1)));
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
            },
            &mut mgr,
            &platform,
        );

        // Window 2 should be added and become focused.
        assert_eq!(mgr.active_workspace().monocle.len(), 2);
        assert_eq!(mgr.active_workspace().monocle.focused(), Some(wid(2)));

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
        assert_eq!(mgr.active_workspace().monocle.len(), 2);
        // Focus should promote to wid(2).
        assert_eq!(mgr.active_workspace().monocle.focused(), Some(wid(2)));

        // Should have focused the promoted window.
        assert!(platform.focus_calls().contains(&wid(2)));
    }

    #[test]
    fn test_manager_loop_quit_command() {
        let platform = MockPlatform::new();
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
            &platform,
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
        let platform = MockPlatform::new();
        let config = test_config();
        let (_hotkey_tx, hotkey_rx) = crossbeam_channel::bounded(16);
        let (_ax_tx, ax_rx) = crossbeam_channel::bounded::<AxEvent>(16);
        let (_ipc_tx, ipc_rx) = crossbeam_channel::bounded::<IpcMessage>(16);
        let shutdown_flag = Arc::new(AtomicBool::new(true)); // Already set.

        manager_loop(
            hotkey_rx,
            ax_rx,
            ipc_rx,
            &platform,
            &config,
            screen(),
            vec![],
            shutdown_flag,
        );

        // Should exit immediately due to shutdown flag.
    }

    #[test]
    fn test_manager_loop_ipc_quit() {
        let platform = MockPlatform::new();
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
            &platform,
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
        let platform = MockPlatform::new();
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
            &platform,
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
        assert!(!ws.monocle.windows().contains(&wid(2)));
    }
}
