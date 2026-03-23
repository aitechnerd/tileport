use crate::monocle::MonocleLayout;
use crate::types::{Gaps, Rect, WindowId};

/// A workspace holds a monocle layout and a set of floating windows.
#[derive(Debug)]
pub struct Workspace {
    /// Workspace number (1-9).
    pub id: u8,
    /// The monocle layout for tiled windows.
    pub monocle: MonocleLayout,
    /// Windows that have been floated out of the monocle layout.
    pub floating_windows: Vec<WindowId>,
    /// Remembered floating positions (NH-01), indexed parallel to `floating_windows`.
    pub floating_rects: Vec<Option<Rect>>,
    /// Per-window fullscreen flag. Only applies to monocle windows.
    pub fullscreen_windows: Vec<WindowId>,
}

impl Workspace {
    /// Create a new empty workspace.
    pub fn new(id: u8) -> Self {
        Self {
            id,
            monocle: MonocleLayout::new(),
            floating_windows: Vec::new(),
            floating_rects: Vec::new(),
            fullscreen_windows: Vec::new(),
        }
    }

    /// Whether a window is in fullscreen mode.
    pub fn is_fullscreen(&self, id: WindowId) -> bool {
        self.fullscreen_windows.contains(&id)
    }

    /// Whether a window is floating.
    pub fn is_floating(&self, id: WindowId) -> bool {
        self.floating_windows.contains(&id)
    }

    /// All windows in this workspace (monocle + floating).
    pub fn all_windows(&self) -> Vec<WindowId> {
        let mut all: Vec<WindowId> = self.monocle.windows().to_vec();
        all.extend_from_slice(&self.floating_windows);
        all
    }

    /// Whether this workspace contains the given window.
    pub fn contains(&self, id: WindowId) -> bool {
        self.monocle.windows().contains(&id) || self.floating_windows.contains(&id)
    }

    /// Remove a window from this workspace entirely (monocle, floating, fullscreen).
    /// Returns the new focused monocle window if the monocle changed.
    pub fn remove_window(&mut self, id: WindowId) -> Option<WindowId> {
        // Remove from floating if present.
        if let Some(pos) = self.floating_windows.iter().position(|w| *w == id) {
            self.floating_windows.remove(pos);
            self.floating_rects.remove(pos);
            self.fullscreen_windows.retain(|w| *w != id);
            return self.monocle.focused();
        }

        // Remove from monocle.
        self.fullscreen_windows.retain(|w| *w != id);
        self.monocle.remove_window(id)
    }
}

/// Result of a workspace switch or window move operation.
///
/// Contains the list of window positions the platform layer should apply.
#[derive(Debug, Clone)]
pub struct WorkspaceTransition {
    /// Windows to move (id, target rect). The platform layer iterates this
    /// list and calls move_window for each entry.
    pub moves: Vec<(WindowId, Rect)>,
}

/// Manages all 9 workspaces and tracks the active one.
#[derive(Debug)]
pub struct WorkspaceManager {
    workspaces: Vec<Workspace>,
    active_index: usize,
    screen: Rect,
    gaps: Gaps,
}

impl WorkspaceManager {
    /// Create a workspace manager with 9 empty workspaces. Workspace 1 is active.
    pub fn new() -> Self {
        let workspaces: Vec<Workspace> = (1..=9).map(Workspace::new).collect();
        Self {
            workspaces,
            active_index: 0,
            screen: Rect {
                x: 0.0,
                y: 0.0,
                width: 1920.0,
                height: 1080.0,
            },
            gaps: Gaps::default(),
        }
    }

    /// Update the screen dimensions and gap configuration.
    pub fn set_screen_and_gaps(&mut self, screen: Rect, gaps: Gaps) {
        self.screen = screen;
        self.gaps = gaps;
    }

    /// The currently active workspace index (0-based internally, workspace 1-9 for users).
    pub fn active_index(&self) -> usize {
        self.active_index
    }

    /// Reference to the active workspace.
    pub fn active_workspace(&self) -> &Workspace {
        &self.workspaces[self.active_index]
    }

    /// Mutable reference to the active workspace.
    pub fn active_workspace_mut(&mut self) -> &mut Workspace {
        &mut self.workspaces[self.active_index]
    }

    /// Reference to a workspace by 1-based number (1-9).
    ///
    /// # Panics
    /// Panics if `number` is 0 or greater than 9.
    pub fn workspace(&self, number: u8) -> &Workspace {
        assert!((1..=9).contains(&number), "workspace number must be 1-9, got {number}");
        &self.workspaces[(number - 1) as usize]
    }

    /// Add a window to the active workspace's monocle layout.
    pub fn add_window(&mut self, id: WindowId) {
        self.workspaces[self.active_index].monocle.add_window(id);
    }

    /// Remove a window from whichever workspace contains it.
    pub fn remove_window(&mut self, id: WindowId) {
        for ws in &mut self.workspaces {
            if ws.contains(id) {
                ws.remove_window(id);
                return;
            }
        }
    }

    /// Switch to a different workspace (1-based target, 1-9).
    ///
    /// Returns a transition describing which windows to hide (move offscreen)
    /// and which to show (move to their layout positions).
    pub fn switch_workspace(&mut self, target: u8) -> WorkspaceTransition {
        if !(1..=9).contains(&target) {
            tracing::warn!(target, "workspace number out of range (1-9), ignoring");
            return WorkspaceTransition { moves: vec![] };
        }

        let target_index = (target - 1) as usize;

        if target_index == self.active_index {
            return WorkspaceTransition { moves: vec![] };
        }

        let mut moves = Vec::new();

        // Hide all windows on current workspace (move offscreen).
        let current_windows = self.workspaces[self.active_index].all_windows();
        for id in current_windows {
            moves.push((
                id,
                Rect {
                    x: self.screen.x + 10000.0,
                    y: self.screen.y + 10000.0,
                    width: self.screen.width,
                    height: self.screen.height,
                },
            ));
        }

        // Show windows on target workspace at their layout positions.
        let target_ws = &self.workspaces[target_index];
        let fullscreen_focused = target_ws
            .monocle
            .focused()
            .map(|id| target_ws.is_fullscreen(id))
            .unwrap_or(false);
        let monocle_positions =
            target_ws
                .monocle
                .calculate_positions(self.screen, self.gaps, fullscreen_focused);
        moves.extend(monocle_positions);

        // Floating windows keep their remembered positions (not moved by layout).
        // They just need to be made visible — for now we don't move them.

        self.active_index = target_index;

        WorkspaceTransition { moves }
    }

    /// Move the focused window from the active workspace to a target workspace (1-based).
    ///
    /// The window is removed from the source monocle and added to the target monocle.
    /// Returns a transition for re-laying out the source workspace.
    pub fn move_window_to_workspace(&mut self, target: u8) -> WorkspaceTransition {
        if !(1..=9).contains(&target) {
            tracing::warn!(target, "workspace number out of range (1-9), ignoring");
            return WorkspaceTransition { moves: vec![] };
        }

        let target_index = (target - 1) as usize;

        if target_index == self.active_index {
            return WorkspaceTransition { moves: vec![] };
        }

        // Get the focused window from the active workspace.
        let focused_id = match self.workspaces[self.active_index].monocle.focused() {
            Some(id) => id,
            None => return WorkspaceTransition { moves: vec![] },
        };

        // Remove from source.
        self.workspaces[self.active_index]
            .monocle
            .remove_window(focused_id);
        self.workspaces[self.active_index]
            .fullscreen_windows
            .retain(|w| *w != focused_id);

        // Add to target.
        self.workspaces[target_index].monocle.add_window(focused_id);

        // Move the window offscreen (it's going to a non-active workspace).
        let mut moves = vec![(
            focused_id,
            Rect {
                x: self.screen.x + 10000.0,
                y: self.screen.y + 10000.0,
                width: self.screen.width,
                height: self.screen.height,
            },
        )];

        // Re-layout current workspace.
        let current_ws = &self.workspaces[self.active_index];
        let fullscreen_focused = current_ws
            .monocle
            .focused()
            .map(|id| current_ws.is_fullscreen(id))
            .unwrap_or(false);
        let positions =
            current_ws
                .monocle
                .calculate_positions(self.screen, self.gaps, fullscreen_focused);
        moves.extend(positions);

        WorkspaceTransition { moves }
    }

    /// Toggle floating state for a window.
    ///
    /// If the window is tiled (in monocle), removes it from monocle and adds
    /// to the floating set. If already floating, returns it to monocle at end.
    pub fn toggle_float(&mut self, id: WindowId) -> WorkspaceTransition {
        let ws = &mut self.workspaces[self.active_index];

        if ws.is_floating(id) {
            // Unfloat: remove from floating, add back to monocle.
            if let Some(pos) = ws.floating_windows.iter().position(|w| *w == id) {
                ws.floating_windows.remove(pos);
                ws.floating_rects.remove(pos);
            }
            ws.monocle.add_window(id);
        } else if ws.monocle.windows().contains(&id) {
            // Float: remove from monocle, add to floating.
            ws.monocle.remove_window(id);
            ws.fullscreen_windows.retain(|w| *w != id);
            ws.floating_windows.push(id);
            // Capture current rect as floating_rect (NH-01). Since we don't
            // have the actual window position in pure Rust, store None. The
            // platform layer will set this when it processes the transition.
            ws.floating_rects.push(None);
        }

        // Return monocle layout positions for the active workspace.
        let ws = &self.workspaces[self.active_index];
        let fullscreen_focused = ws
            .monocle
            .focused()
            .map(|fid| ws.is_fullscreen(fid))
            .unwrap_or(false);
        let moves = ws
            .monocle
            .calculate_positions(self.screen, self.gaps, fullscreen_focused);
        WorkspaceTransition { moves }
    }

    /// Toggle fullscreen for a window in the active workspace.
    ///
    /// Returns a transition with recalculated positions.
    pub fn toggle_fullscreen(&mut self, id: WindowId) -> WorkspaceTransition {
        let ws = &mut self.workspaces[self.active_index];

        if ws.is_fullscreen(id) {
            ws.fullscreen_windows.retain(|w| *w != id);
        } else {
            ws.fullscreen_windows.push(id);
        }

        let ws = &self.workspaces[self.active_index];
        let fullscreen_focused = ws
            .monocle
            .focused()
            .map(|fid| ws.is_fullscreen(fid))
            .unwrap_or(false);
        let moves = ws
            .monocle
            .calculate_positions(self.screen, self.gaps, fullscreen_focused);
        WorkspaceTransition { moves }
    }

    /// Recalculate positions for the active workspace.
    ///
    /// Returns a transition with the current layout positions for all monocle
    /// windows on the active workspace. Used after focus changes, window
    /// add/remove, and other operations that need a layout refresh.
    pub fn recalculate_active(&self) -> WorkspaceTransition {
        let ws = &self.workspaces[self.active_index];
        let fullscreen_focused = ws
            .monocle
            .focused()
            .map(|id| ws.is_fullscreen(id))
            .unwrap_or(false);
        let moves = ws
            .monocle
            .calculate_positions(self.screen, self.gaps, fullscreen_focused);
        WorkspaceTransition { moves }
    }

    /// Get positions for ALL windows across ALL workspaces at visible positions.
    ///
    /// Used during shutdown to restore every window to an on-screen position.
    pub fn get_all_window_positions(&self) -> Vec<(WindowId, Rect)> {
        let mut positions = Vec::new();

        for ws in &self.workspaces {
            // All monocle windows get the gapped screen rect (not offscreen).
            // During shutdown, every window should be visible.
            for &id in ws.monocle.windows() {
                let rect = Rect {
                    x: self.screen.x + self.gaps.outer,
                    y: self.screen.y + self.gaps.outer,
                    width: self.screen.width - 2.0 * self.gaps.outer,
                    height: self.screen.height - 2.0 * self.gaps.outer,
                };
                positions.push((id, rect));
            }

            // Floating windows get their remembered rect, or the gapped rect.
            for (i, &id) in ws.floating_windows.iter().enumerate() {
                let rect = ws.floating_rects[i].unwrap_or(Rect {
                    x: self.screen.x + self.gaps.outer,
                    y: self.screen.y + self.gaps.outer,
                    width: self.screen.width - 2.0 * self.gaps.outer,
                    height: self.screen.height - 2.0 * self.gaps.outer,
                });
                positions.push((id, rect));
            }
        }

        positions
    }
}

impl Default for WorkspaceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn gaps() -> Gaps {
        Gaps {
            inner: 8.0,
            outer: 10.0,
        }
    }

    fn manager() -> WorkspaceManager {
        let mut mgr = WorkspaceManager::new();
        mgr.set_screen_and_gaps(screen(), gaps());
        mgr
    }

    #[test]
    fn test_initial_state() {
        let mgr = manager();
        assert_eq!(mgr.active_index(), 0);
        // 9 workspaces, all empty.
        for i in 1..=9u8 {
            let ws = mgr.workspace(i);
            assert_eq!(ws.id, i);
            assert!(ws.monocle.is_empty());
            assert!(ws.floating_windows.is_empty());
        }
    }

    #[test]
    fn test_add_window_to_active() {
        let mut mgr = manager();
        mgr.add_window(wid(1));
        assert_eq!(mgr.active_workspace().monocle.len(), 1);
        assert_eq!(mgr.active_workspace().monocle.focused(), Some(wid(1)));
    }

    #[test]
    fn test_switch_workspace() {
        // AC-04: switching workspace hides current windows, shows target windows.
        let mut mgr = manager();
        mgr.add_window(wid(1));
        mgr.add_window(wid(2));

        let transition = mgr.switch_workspace(3);
        assert_eq!(mgr.active_index(), 2); // 0-based index for workspace 3

        // Should have moves: hide wid(1) offscreen, hide wid(2) offscreen,
        // plus target workspace monocle positions (empty, so none from there).
        assert_eq!(transition.moves.len(), 2);
        for (_, rect) in &transition.moves {
            assert_eq!(rect.x, 10000.0);
        }
    }

    #[test]
    fn test_switch_to_empty_workspace() {
        // AC-05: switching to empty workspace hides current, no windows to show.
        let mut mgr = manager();
        mgr.add_window(wid(1));

        let transition = mgr.switch_workspace(5);
        assert_eq!(mgr.active_index(), 4);
        // Only the hide move for wid(1).
        assert_eq!(transition.moves.len(), 1);
        assert_eq!(transition.moves[0].0, wid(1));
        assert_eq!(transition.moves[0].1.x, 10000.0);
    }

    #[test]
    fn test_switch_to_same_workspace() {
        let mut mgr = manager();
        mgr.add_window(wid(1));

        let transition = mgr.switch_workspace(1);
        assert!(transition.moves.is_empty());
        assert_eq!(mgr.active_index(), 0);
    }

    #[test]
    fn test_move_window_to_workspace() {
        // AC-06: move focused window to another workspace.
        let mut mgr = manager();
        mgr.add_window(wid(1));
        mgr.add_window(wid(2));
        // focused = wid(2)

        let transition = mgr.move_window_to_workspace(3);

        // wid(2) should now be in workspace 3.
        assert!(mgr.workspace(3).monocle.windows().contains(&wid(2)));
        // wid(2) should no longer be in workspace 1.
        assert!(!mgr.workspace(1).monocle.windows().contains(&wid(2)));
        // wid(1) should still be in workspace 1.
        assert!(mgr.workspace(1).monocle.windows().contains(&wid(1)));

        // Transition should include: wid(2) offscreen + wid(1) layout position.
        assert!(!transition.moves.is_empty());
        // wid(2) moved offscreen.
        let move_2 = transition.moves.iter().find(|(id, _)| *id == wid(2)).unwrap();
        assert_eq!(move_2.1.x, 10000.0);
    }

    #[test]
    fn test_move_last_window_leaves_empty() {
        let mut mgr = manager();
        mgr.add_window(wid(1));

        mgr.move_window_to_workspace(2);

        assert!(mgr.workspace(1).monocle.is_empty());
        assert_eq!(mgr.workspace(2).monocle.len(), 1);
        assert!(mgr.workspace(2).monocle.windows().contains(&wid(1)));
    }

    #[test]
    fn test_toggle_float() {
        // AC-07: floating window exits monocle.
        let mut mgr = manager();
        mgr.add_window(wid(1));
        mgr.add_window(wid(2));

        mgr.toggle_float(wid(2));

        let ws = mgr.active_workspace();
        assert!(!ws.monocle.windows().contains(&wid(2)));
        assert!(ws.floating_windows.contains(&wid(2)));
        assert_eq!(ws.monocle.len(), 1);
        assert_eq!(ws.monocle.focused(), Some(wid(1)));
    }

    #[test]
    fn test_toggle_float_off() {
        // AC-08: unfloating returns window to monocle.
        let mut mgr = manager();
        mgr.add_window(wid(1));
        mgr.add_window(wid(2));

        // Float then unfloat.
        mgr.toggle_float(wid(2));
        mgr.toggle_float(wid(2));

        let ws = mgr.active_workspace();
        assert!(ws.monocle.windows().contains(&wid(2)));
        assert!(!ws.floating_windows.contains(&wid(2)));
        // Unfloated window is added at end and becomes focused.
        assert_eq!(ws.monocle.focused(), Some(wid(2)));
    }

    #[test]
    fn test_toggle_fullscreen() {
        // AC-09: fullscreen toggles zero-gap positions.
        let mut mgr = manager();
        mgr.add_window(wid(1));

        let transition = mgr.toggle_fullscreen(wid(1));

        assert!(mgr.active_workspace().is_fullscreen(wid(1)));
        // The focused window should get zero-gap (full screen) rect.
        let (_, rect) = transition
            .moves
            .iter()
            .find(|(id, _)| *id == wid(1))
            .unwrap();
        assert_eq!(rect.x, 0.0);
        assert_eq!(rect.y, 0.0);
        assert_eq!(rect.width, 1920.0);
        assert_eq!(rect.height, 1080.0);

        // Toggle off.
        let transition = mgr.toggle_fullscreen(wid(1));
        assert!(!mgr.active_workspace().is_fullscreen(wid(1)));
        let (_, rect) = transition
            .moves
            .iter()
            .find(|(id, _)| *id == wid(1))
            .unwrap();
        // Should now have gaps.
        assert_eq!(rect.x, 10.0);
        assert_eq!(rect.width, 1900.0);
    }

    #[test]
    fn test_remove_window_any_workspace() {
        let mut mgr = manager();
        mgr.add_window(wid(1));
        // Move wid(1) to workspace 3, then switch back.
        mgr.move_window_to_workspace(3);

        // Remove wid(1) from wherever it is.
        mgr.remove_window(wid(1));

        assert!(mgr.workspace(3).monocle.is_empty());
    }

    #[test]
    fn test_get_all_window_positions_for_shutdown() {
        let mut mgr = manager();

        // Add windows to workspace 1.
        mgr.add_window(wid(1));
        mgr.add_window(wid(2));

        // Move wid(2) to workspace 3.
        mgr.move_window_to_workspace(3);

        // Float wid(1).
        mgr.toggle_float(wid(1));

        // Add another window to workspace 1.
        mgr.add_window(wid(3));

        let positions = mgr.get_all_window_positions();

        // Should have positions for wid(1) (floating), wid(2) (ws3 monocle), wid(3) (ws1 monocle).
        assert_eq!(positions.len(), 3);

        // All windows should be at visible (on-screen) positions.
        for (_, rect) in &positions {
            assert!(rect.x < 10000.0, "shutdown positions should be on-screen");
        }
    }

    #[test]
    fn test_switch_workspace_zero_returns_noop() {
        let mut mgr = manager();
        mgr.add_window(wid(1));

        let transition = mgr.switch_workspace(0);
        assert!(transition.moves.is_empty(), "workspace 0 should be a no-op");
        assert_eq!(mgr.active_index(), 0, "active workspace should not change");
    }

    #[test]
    fn test_switch_workspace_ten_returns_noop() {
        let mut mgr = manager();
        mgr.add_window(wid(1));

        let transition = mgr.switch_workspace(10);
        assert!(transition.moves.is_empty(), "workspace 10 should be a no-op");
        assert_eq!(mgr.active_index(), 0, "active workspace should not change");
    }

    #[test]
    fn test_move_window_to_workspace_zero_returns_noop() {
        let mut mgr = manager();
        mgr.add_window(wid(1));

        let transition = mgr.move_window_to_workspace(0);
        assert!(transition.moves.is_empty(), "workspace 0 should be a no-op");
        // Window should still be in workspace 1.
        assert!(mgr.workspace(1).monocle.windows().contains(&wid(1)));
    }

    #[test]
    fn test_move_window_to_workspace_ten_returns_noop() {
        let mut mgr = manager();
        mgr.add_window(wid(1));

        let transition = mgr.move_window_to_workspace(10);
        assert!(transition.moves.is_empty(), "workspace 10 should be a no-op");
        assert!(mgr.workspace(1).monocle.windows().contains(&wid(1)));
    }
}
