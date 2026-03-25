use crate::monocle::MonocleLayout;
use crate::types::{Gaps, Rect, WindowId};
use crate::zone::{Direction, ZoneLayout};

/// Layout type for a workspace.
#[derive(Debug)]
pub enum WorkspaceLayout {
    Monocle(MonocleLayout),
    Zone(ZoneLayout),
}

impl WorkspaceLayout {
    /// Add a window to the layout. The new window becomes focused.
    pub fn add_window(&mut self, id: WindowId) {
        match self {
            WorkspaceLayout::Monocle(m) => m.add_window(id),
            WorkspaceLayout::Zone(z) => z.add_window(id),
        }
    }

    /// Remove a window from the layout.
    /// Returns the new focused window, or `None` if the layout is now empty.
    pub fn remove_window(&mut self, id: WindowId) -> Option<WindowId> {
        match self {
            WorkspaceLayout::Monocle(m) => m.remove_window(id),
            WorkspaceLayout::Zone(z) => z.remove_window(id),
        }
    }

    /// The currently focused window, or `None` if the layout is empty.
    pub fn focused(&self) -> Option<WindowId> {
        match self {
            WorkspaceLayout::Monocle(m) => m.focused(),
            WorkspaceLayout::Zone(z) => z.focused(),
        }
    }

    /// All windows in the layout.
    pub fn windows(&self) -> Vec<WindowId> {
        match self {
            WorkspaceLayout::Monocle(m) => m.windows().to_vec(),
            WorkspaceLayout::Zone(z) => z.windows(),
        }
    }

    /// Number of windows in the layout.
    pub fn len(&self) -> usize {
        match self {
            WorkspaceLayout::Monocle(m) => m.len(),
            WorkspaceLayout::Zone(z) => z.len(),
        }
    }

    /// Whether the layout has no windows.
    pub fn is_empty(&self) -> bool {
        match self {
            WorkspaceLayout::Monocle(m) => m.is_empty(),
            WorkspaceLayout::Zone(z) => z.is_empty(),
        }
    }

    /// Calculate the position for every window in the layout.
    pub fn calculate_positions(
        &self,
        screen: Rect,
        gaps: Gaps,
        fullscreen: bool,
    ) -> Vec<(WindowId, Rect)> {
        match self {
            WorkspaceLayout::Monocle(m) => m.calculate_positions(screen, gaps, fullscreen),
            WorkspaceLayout::Zone(z) => z.calculate_positions(screen, gaps, fullscreen),
        }
    }

    /// Move focus to the next window.
    pub fn focus_next(&mut self) -> Option<WindowId> {
        match self {
            WorkspaceLayout::Monocle(m) => m.focus_next(),
            WorkspaceLayout::Zone(z) => z.focus_next(),
        }
    }

    /// Move focus to the previous window.
    pub fn focus_prev(&mut self) -> Option<WindowId> {
        match self {
            WorkspaceLayout::Monocle(m) => m.focus_prev(),
            WorkspaceLayout::Zone(z) => z.focus_prev(),
        }
    }

    /// Move focus in a direction.
    ///
    /// For monocle: Left/Up -> focus_prev, Right/Down -> focus_next.
    /// For zone: delegate to ZoneLayout::focus_direction.
    pub fn focus_direction(&mut self, dir: Direction) -> Option<WindowId> {
        match self {
            WorkspaceLayout::Monocle(m) => match dir {
                Direction::Left | Direction::Up => m.focus_prev(),
                Direction::Right | Direction::Down => m.focus_next(),
            },
            WorkspaceLayout::Zone(z) => z.focus_direction(dir),
        }
    }

    /// Move the focused window to the adjacent zone in the given direction.
    ///
    /// Monocle: no-op (returns false).
    /// Zone: delegate to ZoneLayout::move_to_zone.
    pub fn move_to_zone(&mut self, dir: Direction) -> bool {
        match self {
            WorkspaceLayout::Monocle(_) => false,
            WorkspaceLayout::Zone(z) => z.move_to_zone(dir),
        }
    }

    /// Swap the focused window with the primary zone's visible window.
    ///
    /// Monocle: no-op (returns false).
    /// Zone: delegate to ZoneLayout::promote_to_primary.
    pub fn promote_to_primary(&mut self) -> bool {
        match self {
            WorkspaceLayout::Monocle(_) => false,
            WorkspaceLayout::Zone(z) => z.promote_to_primary(),
        }
    }
}

/// A workspace holds a layout and a set of floating windows.
#[derive(Debug)]
pub struct Workspace {
    /// Workspace number (1-9).
    pub id: u8,
    /// The layout for tiled windows.
    pub layout: WorkspaceLayout,
    /// Windows that have been floated out of the layout.
    pub floating_windows: Vec<WindowId>,
    /// Remembered floating positions (NH-01), indexed parallel to `floating_windows`.
    pub floating_rects: Vec<Option<Rect>>,
    /// Per-window fullscreen flag. Only applies to tiled windows.
    pub fullscreen_windows: Vec<WindowId>,
}

impl Workspace {
    /// Create a new empty workspace with monocle layout.
    pub fn new(id: u8) -> Self {
        Self {
            id,
            layout: WorkspaceLayout::Monocle(MonocleLayout::new()),
            floating_windows: Vec::new(),
            floating_rects: Vec::new(),
            fullscreen_windows: Vec::new(),
        }
    }

    /// Create a new empty workspace with a specific layout.
    pub fn new_with_layout(id: u8, layout: WorkspaceLayout) -> Self {
        Self {
            id,
            layout,
            floating_windows: Vec::new(),
            floating_rects: Vec::new(),
            fullscreen_windows: Vec::new(),
        }
    }

    /// Convenience: get MonocleLayout ref (panics if not monocle).
    /// Used only in tests for backward compat.
    #[cfg(test)]
    pub fn monocle(&self) -> &MonocleLayout {
        match &self.layout {
            WorkspaceLayout::Monocle(m) => m,
            _ => panic!("expected monocle layout"),
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

    /// All windows in this workspace (layout + floating).
    pub fn all_windows(&self) -> Vec<WindowId> {
        let mut all: Vec<WindowId> = self.layout.windows();
        all.extend_from_slice(&self.floating_windows);
        all
    }

    /// Whether this workspace contains the given window.
    pub fn contains(&self, id: WindowId) -> bool {
        self.layout.windows().contains(&id) || self.floating_windows.contains(&id)
    }

    /// Remove a window from this workspace entirely (layout, floating, fullscreen).
    /// Returns the new focused window if the layout changed.
    pub fn remove_window(&mut self, id: WindowId) -> Option<WindowId> {
        // Remove from floating if present.
        if let Some(pos) = self.floating_windows.iter().position(|w| *w == id) {
            self.floating_windows.remove(pos);
            self.floating_rects.remove(pos);
            self.fullscreen_windows.retain(|w| *w != id);
            return self.layout.focused();
        }

        // Remove from layout.
        self.fullscreen_windows.retain(|w| *w != id);
        self.layout.remove_window(id)
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
    /// Shared initializer: wraps a pre-built workspace vec with default screen/gaps.
    fn from_workspaces(workspaces: Vec<Workspace>) -> Self {
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

    /// Create a workspace manager with 9 empty workspaces. Workspace 1 is active.
    pub fn new() -> Self {
        Self::from_workspaces((1..=9).map(Workspace::new).collect())
    }

    /// Create a workspace manager with specific layouts for some workspaces.
    ///
    /// Workspaces not in the map get the default monocle layout.
    pub fn new_with_layouts(mut layouts: std::collections::HashMap<u8, WorkspaceLayout>) -> Self {
        let workspaces = (1..=9u8)
            .map(|id| match layouts.remove(&id) {
                Some(layout) => Workspace::new_with_layout(id, layout),
                None => Workspace::new(id),
            })
            .collect();
        Self::from_workspaces(workspaces)
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

    /// Mutable reference to a workspace by 1-based number (1-9).
    ///
    /// # Panics
    /// Panics if `number` is 0 or greater than 9.
    pub fn workspace_mut(&mut self, number: u8) -> &mut Workspace {
        assert!((1..=9).contains(&number), "workspace number must be 1-9, got {number}");
        &mut self.workspaces[(number - 1) as usize]
    }

    /// Add a window to the active workspace's layout.
    pub fn add_window(&mut self, id: WindowId) {
        self.workspaces[self.active_index].layout.add_window(id);
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
            .layout
            .focused()
            .map(|id| target_ws.is_fullscreen(id))
            .unwrap_or(false);
        let layout_positions =
            target_ws
                .layout
                .calculate_positions(self.screen, self.gaps, fullscreen_focused);
        moves.extend(layout_positions);

        // Floating windows keep their remembered positions (not moved by layout).
        // They just need to be made visible -- for now we don't move them.

        self.active_index = target_index;

        WorkspaceTransition { moves }
    }

    /// Move the focused window from the active workspace to a target workspace (1-based).
    ///
    /// The window is removed from the source layout and added to the target layout.
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
        let focused_id = match self.workspaces[self.active_index].layout.focused() {
            Some(id) => id,
            None => return WorkspaceTransition { moves: vec![] },
        };

        // Remove from source.
        self.workspaces[self.active_index]
            .layout
            .remove_window(focused_id);
        self.workspaces[self.active_index]
            .fullscreen_windows
            .retain(|w| *w != focused_id);

        // Add to target.
        self.workspaces[target_index].layout.add_window(focused_id);

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
            .layout
            .focused()
            .map(|id| current_ws.is_fullscreen(id))
            .unwrap_or(false);
        let positions =
            current_ws
                .layout
                .calculate_positions(self.screen, self.gaps, fullscreen_focused);
        moves.extend(positions);

        WorkspaceTransition { moves }
    }

    /// Toggle floating state for a window.
    ///
    /// If the window is tiled (in layout), removes it from layout and adds
    /// to the floating set. If already floating, returns it to layout at end.
    pub fn toggle_float(&mut self, id: WindowId) -> WorkspaceTransition {
        let ws = &mut self.workspaces[self.active_index];

        if ws.is_floating(id) {
            // Unfloat: remove from floating, add back to layout.
            if let Some(pos) = ws.floating_windows.iter().position(|w| *w == id) {
                ws.floating_windows.remove(pos);
                ws.floating_rects.remove(pos);
            }
            ws.layout.add_window(id);
        } else if ws.layout.windows().contains(&id) {
            // Float: remove from layout, add to floating.
            ws.layout.remove_window(id);
            ws.fullscreen_windows.retain(|w| *w != id);
            ws.floating_windows.push(id);
            // Capture current rect as floating_rect (NH-01). Since we don't
            // have the actual window position in pure Rust, store None. The
            // platform layer will set this when it processes the transition.
            ws.floating_rects.push(None);
        }

        // Return layout positions for the active workspace.
        let ws = &self.workspaces[self.active_index];
        let fullscreen_focused = ws
            .layout
            .focused()
            .map(|fid| ws.is_fullscreen(fid))
            .unwrap_or(false);
        let moves = ws
            .layout
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
            .layout
            .focused()
            .map(|fid| ws.is_fullscreen(fid))
            .unwrap_or(false);
        let moves = ws
            .layout
            .calculate_positions(self.screen, self.gaps, fullscreen_focused);
        WorkspaceTransition { moves }
    }

    /// Recalculate positions for the active workspace.
    ///
    /// Returns a transition with the current layout positions for all tiled
    /// windows on the active workspace. Used after focus changes, window
    /// add/remove, and other operations that need a layout refresh.
    pub fn recalculate_active(&self) -> WorkspaceTransition {
        let ws = &self.workspaces[self.active_index];
        let fullscreen_focused = ws
            .layout
            .focused()
            .map(|id| ws.is_fullscreen(id))
            .unwrap_or(false);
        let moves = ws
            .layout
            .calculate_positions(self.screen, self.gaps, fullscreen_focused);
        WorkspaceTransition { moves }
    }

    /// Move focus in a direction on the active workspace.
    ///
    /// Delegates to WorkspaceLayout::focus_direction.
    pub fn focus_direction(&mut self, dir: Direction) -> Option<WindowId> {
        self.workspaces[self.active_index].layout.focus_direction(dir)
    }

    /// Move the focused window to the adjacent zone in the given direction.
    ///
    /// Delegates to WorkspaceLayout::move_to_zone.
    pub fn move_to_zone(&mut self, dir: Direction) -> bool {
        self.workspaces[self.active_index].layout.move_to_zone(dir)
    }

    /// Swap the focused window with the primary zone's visible window.
    ///
    /// Delegates to WorkspaceLayout::promote_to_primary.
    pub fn promote_to_primary(&mut self) -> bool {
        self.workspaces[self.active_index].layout.promote_to_primary()
    }

    /// Get positions for ALL windows across ALL workspaces at visible positions.
    ///
    /// Used during shutdown to restore every window to an on-screen position.
    pub fn get_all_window_positions(&self) -> Vec<(WindowId, Rect)> {
        let gapped = Rect {
            x: self.screen.x + self.gaps.outer,
            y: self.screen.y + self.gaps.outer,
            width: self.screen.width - 2.0 * self.gaps.outer,
            height: self.screen.height - 2.0 * self.gaps.outer,
        };

        let mut positions = Vec::new();

        for ws in &self.workspaces {
            // All layout windows get the gapped screen rect (not offscreen).
            // During shutdown, every window should be visible.
            for id in ws.layout.windows() {
                positions.push((id, gapped));
            }

            // Floating windows get their remembered rect, or the gapped rect.
            for (i, &id) in ws.floating_windows.iter().enumerate() {
                positions.push((id, ws.floating_rects[i].unwrap_or(gapped)));
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
    use crate::zone::{make_fill_order, ZoneNode};

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

    /// Create a 2-column zone layout (50/50) for testing.
    fn two_col_zone_layout() -> WorkspaceLayout {
        let root = ZoneNode::HSplit {
            ratios: vec![0.5, 0.5],
            children: vec![ZoneNode::Leaf, ZoneNode::Leaf],
        };
        let fill_order = make_fill_order(&root);
        let primary_zone = fill_order[0];
        WorkspaceLayout::Zone(ZoneLayout::new(root, fill_order, primary_zone))
    }

    #[test]
    fn test_initial_state() {
        let mgr = manager();
        assert_eq!(mgr.active_index(), 0);
        // 9 workspaces, all empty.
        for i in 1..=9u8 {
            let ws = mgr.workspace(i);
            assert_eq!(ws.id, i);
            assert!(ws.monocle().is_empty());
            assert!(ws.floating_windows.is_empty());
        }
    }

    #[test]
    fn test_add_window_to_active() {
        let mut mgr = manager();
        mgr.add_window(wid(1));
        assert_eq!(mgr.active_workspace().monocle().len(), 1);
        assert_eq!(mgr.active_workspace().monocle().focused(), Some(wid(1)));
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
        assert!(mgr.workspace(3).monocle().windows().contains(&wid(2)));
        // wid(2) should no longer be in workspace 1.
        assert!(!mgr.workspace(1).monocle().windows().contains(&wid(2)));
        // wid(1) should still be in workspace 1.
        assert!(mgr.workspace(1).monocle().windows().contains(&wid(1)));

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

        assert!(mgr.workspace(1).monocle().is_empty());
        assert_eq!(mgr.workspace(2).monocle().len(), 1);
        assert!(mgr.workspace(2).monocle().windows().contains(&wid(1)));
    }

    #[test]
    fn test_toggle_float() {
        // AC-07: floating window exits monocle.
        let mut mgr = manager();
        mgr.add_window(wid(1));
        mgr.add_window(wid(2));

        mgr.toggle_float(wid(2));

        let ws = mgr.active_workspace();
        assert!(!ws.monocle().windows().contains(&wid(2)));
        assert!(ws.floating_windows.contains(&wid(2)));
        assert_eq!(ws.monocle().len(), 1);
        assert_eq!(ws.monocle().focused(), Some(wid(1)));
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
        assert!(ws.monocle().windows().contains(&wid(2)));
        assert!(!ws.floating_windows.contains(&wid(2)));
        // Unfloated window is added at end and becomes focused.
        assert_eq!(ws.monocle().focused(), Some(wid(2)));
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

        assert!(mgr.workspace(3).monocle().is_empty());
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
        assert!(mgr.workspace(1).monocle().windows().contains(&wid(1)));
    }

    #[test]
    fn test_move_window_to_workspace_ten_returns_noop() {
        let mut mgr = manager();
        mgr.add_window(wid(1));

        let transition = mgr.move_window_to_workspace(10);
        assert!(transition.moves.is_empty(), "workspace 10 should be a no-op");
        assert!(mgr.workspace(1).monocle().windows().contains(&wid(1)));
    }

    // --- Phase 2: New zone workspace tests ---

    #[test]
    fn test_workspace_with_zone_layout() {
        let layout = two_col_zone_layout();
        let mut ws = Workspace::new_with_layout(1, layout);

        // Add windows -- they fill zones per fill order.
        ws.layout.add_window(wid(1));
        ws.layout.add_window(wid(2));

        assert_eq!(ws.layout.len(), 2);
        assert!(!ws.layout.is_empty());
        assert_eq!(ws.layout.focused(), Some(wid(2)));

        // Both windows should be present.
        let all = ws.layout.windows();
        assert!(all.contains(&wid(1)));
        assert!(all.contains(&wid(2)));

        // Remove a window.
        let new_focused = ws.layout.remove_window(wid(2));
        assert_eq!(ws.layout.len(), 1);
        assert_eq!(new_focused, Some(wid(1)));

        // contains() should work.
        assert!(ws.contains(wid(1)));
        assert!(!ws.contains(wid(2)));
    }

    #[test]
    fn test_workspace_manager_with_mixed_layouts() {
        let mut mgr = manager();

        // Set workspace 2 to zone layout.
        let layout = two_col_zone_layout();
        *mgr.workspace_mut(2) = Workspace::new_with_layout(2, layout);

        // Add windows to workspace 1 (monocle).
        mgr.add_window(wid(1));
        mgr.add_window(wid(2));
        assert_eq!(mgr.active_workspace().layout.len(), 2);

        // Switch to workspace 2 (zone).
        let transition = mgr.switch_workspace(2);
        assert_eq!(mgr.active_index(), 1);

        // Transition should hide ws1 windows.
        let offscreen: Vec<_> = transition
            .moves
            .iter()
            .filter(|(_, r)| r.x >= 10000.0)
            .collect();
        assert_eq!(offscreen.len(), 2);

        // Add windows to workspace 2 (zone layout).
        mgr.add_window(wid(10));
        mgr.add_window(wid(11));
        assert_eq!(mgr.active_workspace().layout.len(), 2);

        // Verify zone positions -- both should be on-screen for zone layout.
        let positions = mgr.recalculate_active();
        for (_, rect) in &positions.moves {
            assert!(rect.x < 10000.0, "zone windows should be on-screen");
        }

        // Switch back to workspace 1 (monocle).
        let transition = mgr.switch_workspace(1);
        assert_eq!(mgr.active_index(), 0);

        // Should hide ws2 zone windows and show ws1 monocle windows.
        assert!(!transition.moves.is_empty());
    }

    #[test]
    fn test_switch_workspace_zone_to_monocle() {
        let mut mgr = manager();

        // Set workspace 1 to zone layout.
        let layout = two_col_zone_layout();
        *mgr.workspace_mut(1) = Workspace::new_with_layout(1, layout);

        // Add windows to zone workspace.
        mgr.add_window(wid(1));
        mgr.add_window(wid(2));

        // Switch to workspace 2 (monocle, empty).
        let transition = mgr.switch_workspace(2);

        // Both zone windows should be hidden.
        let offscreen: Vec<_> = transition
            .moves
            .iter()
            .filter(|(_, r)| r.x >= 10000.0)
            .collect();
        assert_eq!(offscreen.len(), 2, "both zone windows should go offscreen");

        // Switch back to workspace 1 (zone).
        mgr.add_window(wid(10)); // add to ws2 first
        let transition = mgr.switch_workspace(1);

        // ws2 monocle window should go offscreen; ws1 zone windows should show.
        let offscreen: Vec<_> = transition
            .moves
            .iter()
            .filter(|(_, r)| r.x >= 10000.0)
            .collect();
        assert_eq!(offscreen.len(), 1, "ws2 monocle window should go offscreen");

        // Zone windows should be on-screen.
        let onscreen: Vec<_> = transition
            .moves
            .iter()
            .filter(|(_, r)| r.x < 10000.0)
            .collect();
        assert_eq!(onscreen.len(), 2, "ws1 zone windows should be on-screen");
    }

    #[test]
    fn test_get_all_window_positions_zone_workspace() {
        let mut mgr = manager();

        // Set workspace 1 to zone layout.
        let layout = two_col_zone_layout();
        *mgr.workspace_mut(1) = Workspace::new_with_layout(1, layout);

        // Add windows to zone workspace 1.
        mgr.add_window(wid(1));
        mgr.add_window(wid(2));

        // Switch to workspace 2, add a monocle window.
        mgr.switch_workspace(2);
        mgr.add_window(wid(3));

        // Shutdown should restore all windows to visible positions.
        let positions = mgr.get_all_window_positions();

        assert_eq!(positions.len(), 3);
        for (_, rect) in &positions {
            assert!(rect.x < 10000.0, "shutdown positions should be on-screen");
        }
    }
}

/// Integration-style workflow tests that simulate realistic multi-step user sessions.
///
/// Each test exercises the full WorkspaceManager API across multiple operations
/// (add, focus, move, switch, float, remove) to verify end-to-end correctness.
#[cfg(test)]
mod workflow_tests {
    use super::*;
    use crate::config::Config;
    use crate::zone::{make_fill_order, Direction, ZoneLayout, ZoneNode};
    use std::collections::HashMap;

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

    /// Build a 2-column zone layout with 50/50 split.
    fn two_col_layout() -> WorkspaceLayout {
        let root = ZoneNode::HSplit {
            ratios: vec![0.5, 0.5],
            children: vec![ZoneNode::Leaf, ZoneNode::Leaf],
        };
        let fill_order = make_fill_order(&root);
        let primary = fill_order[0];
        WorkspaceLayout::Zone(ZoneLayout::new(root, fill_order, primary))
    }

    /// Build a 3-column zone layout with given ratios.
    fn three_col_layout(ratios: [f64; 3]) -> WorkspaceLayout {
        let root = ZoneNode::HSplit {
            ratios: ratios.to_vec(),
            children: vec![ZoneNode::Leaf, ZoneNode::Leaf, ZoneNode::Leaf],
        };
        let fill_order = make_fill_order(&root);
        let primary = fill_order[0]; // center = 1 for 3-col
        WorkspaceLayout::Zone(ZoneLayout::new(root, fill_order, primary))
    }

    /// Create a WorkspaceManager with specific layouts and standard screen/gaps.
    fn manager_with(layouts: HashMap<u8, WorkspaceLayout>) -> WorkspaceManager {
        let mut mgr = WorkspaceManager::new_with_layouts(layouts);
        mgr.set_screen_and_gaps(screen(), gaps());
        mgr
    }

    /// Find a window's rect in a list of positions.
    fn find_rect(positions: &[(WindowId, Rect)], id: WindowId) -> Option<Rect> {
        positions.iter().find(|(w, _)| *w == id).map(|(_, r)| *r)
    }

    /// Check if a rect is offscreen (x >= 10000).
    fn is_offscreen(r: &Rect) -> bool {
        r.x >= 10000.0
    }

    // -----------------------------------------------------------------------
    // Scenario 1: "Developer workday" flow
    // -----------------------------------------------------------------------
    #[test]
    fn test_workflow_developer_workday() {
        // 1. Create WorkspaceManager with ws1=2-col, ws2=3-col, ws3=monocle
        let mut layouts = HashMap::new();
        layouts.insert(1, two_col_layout());
        layouts.insert(2, three_col_layout([0.30, 0.40, 0.30]));
        // ws3 stays default monocle
        let mut mgr = manager_with(layouts);

        let terminal = wid(1);
        let editor = wid(2);
        let browser = wid(3);
        let slack = wid(4);

        // 2. Add 4 windows to ws1 (simulating Terminal, Editor, Browser, Slack)
        mgr.add_window(terminal);
        mgr.add_window(editor);
        mgr.add_window(browser);
        mgr.add_window(slack);

        // 3. First 2 windows go to ws1 zones (2-col: zone 0 and zone 1)
        //    Browser and Slack overflow into zone 1 (last fill_order slot for 2-col)
        let positions = mgr.recalculate_active();

        // Terminal in zone 0 (left 50%) -- visible
        let term_rect = find_rect(&positions.moves, terminal).unwrap();
        assert!(term_rect.x < 100.0, "terminal should be on left side");
        assert!(!is_offscreen(&term_rect), "terminal should be on-screen");

        // Editor in zone 1 (right 50%) -- visible
        let editor_rect = find_rect(&positions.moves, editor).unwrap();
        assert!(editor_rect.x > 900.0, "editor should be on right side");
        assert!(!is_offscreen(&editor_rect), "editor should be on-screen");

        // Both visible windows have roughly equal width (50/50 minus gaps)
        let width_diff = (term_rect.width - editor_rect.width).abs();
        assert!(width_diff < 1.0, "50/50 split should give equal widths");

        // 4. Focus left (alt+h) -> verify focused window changes
        //    After adding 4 windows to 2-col, focused_zone=1 (last fill_order slot).
        //    Zone 1 = [editor, browser, slack]. focused() = editor (index 0).
        assert_eq!(mgr.active_workspace().layout.focused(), Some(editor));
        mgr.focus_direction(Direction::Left);
        assert_eq!(
            mgr.active_workspace().layout.focused(),
            Some(terminal),
            "focus left from zone 1 should go to zone 0"
        );

        // 5. Focus right back to zone 1
        mgr.focus_direction(Direction::Right);
        // Now focused is in zone 1 (editor is visible there)
        let focused_before_move = mgr.active_workspace().layout.focused().unwrap();

        // Move focused window to ws2 (alt+shift+2)
        let transition = mgr.move_window_to_workspace(2);

        // Verify the moved window goes offscreen
        let moved_rect = find_rect(&transition.moves, focused_before_move).unwrap();
        assert!(is_offscreen(&moved_rect), "moved window should go offscreen");

        // Verify ws1 re-layouts (remaining windows still visible)
        assert!(
            !mgr.workspace(1).layout.windows().contains(&focused_before_move),
            "moved window should not be in ws1"
        );
        assert!(
            mgr.workspace(2).layout.windows().contains(&focused_before_move),
            "moved window should be in ws2"
        );

        // 6. Switch to ws2 (alt+2) -> verify 3-col layout
        let _transition = mgr.switch_workspace(2);
        assert_eq!(mgr.active_index(), 1);

        // The moved window should be in the center zone (center-first fill for 3-col)
        let positions = mgr.recalculate_active();
        let moved_rect = find_rect(&positions.moves, focused_before_move).unwrap();
        assert!(!is_offscreen(&moved_rect), "moved window should be visible on ws2");

        // 7. Add another window on ws2 -> goes to left zone (second fill slot)
        let extra = wid(10);
        mgr.add_window(extra);
        let positions = mgr.recalculate_active();
        let extra_rect = find_rect(&positions.moves, extra).unwrap();
        assert!(!is_offscreen(&extra_rect), "new window should be visible");
        // Left zone (zone index 0) should have smaller x than center zone
        assert!(
            extra_rect.x < moved_rect.x,
            "second fill slot should be left of center"
        );

        // 8. Focus left (alt+h) -> verify focus moves to left zone
        mgr.focus_direction(Direction::Left);
        assert_eq!(
            mgr.active_workspace().layout.focused(),
            Some(extra),
            "focus left should go to the left zone"
        );

        // 9. Promote to primary (alt+enter) -> verify swap with center
        mgr.promote_to_primary();
        // After promote, focused zone becomes primary (center)
        // The extra window should now be in center, moved window in left
        let positions = mgr.recalculate_active();
        let extra_rect_after = find_rect(&positions.moves, extra).unwrap();
        let moved_rect_after = find_rect(&positions.moves, focused_before_move).unwrap();
        // After swap, extra should be where moved was (center) and vice versa
        assert!(
            extra_rect_after.x > moved_rect_after.x,
            "promoted window should be in center (higher x than left zone)"
        );

        // 10. Switch back to ws1 -> verify original windows still there
        mgr.switch_workspace(1);
        assert_eq!(mgr.active_index(), 0);
        let ws1_windows = mgr.workspace(1).layout.windows();
        assert!(
            ws1_windows.contains(&terminal),
            "terminal should still be on ws1"
        );
    }

    // -----------------------------------------------------------------------
    // Scenario 2: "Float and unfloat cycle"
    // -----------------------------------------------------------------------
    #[test]
    fn test_workflow_float_unfloat_cycle() {
        let mut layouts = HashMap::new();
        layouts.insert(1, two_col_layout());
        let mut mgr = manager_with(layouts);

        let win_a = wid(1);
        let win_b = wid(2);

        // 1. 2-col layout, 2 windows in zones
        mgr.add_window(win_a); // zone 0
        mgr.add_window(win_b); // zone 1

        let positions = mgr.recalculate_active();
        assert_eq!(positions.moves.len(), 2);
        let rect_a_before = find_rect(&positions.moves, win_a).unwrap();
        let rect_b_before = find_rect(&positions.moves, win_b).unwrap();
        assert!(!is_offscreen(&rect_a_before));
        assert!(!is_offscreen(&rect_b_before));

        // 2. Float window from zone 0 -> verify zone 0 empty, zone 1 still has its window
        //    Need to focus win_a first to float it
        mgr.focus_direction(Direction::Left); // focus zone 0
        assert_eq!(mgr.active_workspace().layout.focused(), Some(win_a));
        mgr.toggle_float(win_a);

        assert!(mgr.active_workspace().is_floating(win_a), "win_a should be floating");
        assert!(
            !mgr.active_workspace().layout.windows().contains(&win_a),
            "win_a should not be in layout"
        );
        assert!(
            mgr.active_workspace().layout.windows().contains(&win_b),
            "win_b should still be in layout"
        );

        // 3. Add new window -> should fill the now-empty zone 0
        let win_c = wid(3);
        mgr.add_window(win_c);

        let positions = mgr.recalculate_active();
        let rect_c = find_rect(&positions.moves, win_c).unwrap();
        assert!(!is_offscreen(&rect_c), "new window should be visible in zone");

        // 4. Unfloat original window -> should go back into zone layout
        mgr.toggle_float(win_a);
        assert!(!mgr.active_workspace().is_floating(win_a), "win_a should not be floating");
        assert!(
            mgr.active_workspace().layout.windows().contains(&win_a),
            "win_a should be back in layout"
        );

        // 5. Verify all positions correct -- all 3 windows accounted for
        let positions = mgr.recalculate_active();
        let all_visible: Vec<_> = positions
            .moves
            .iter()
            .filter(|(_, r)| !is_offscreen(r))
            .collect();
        // 2-col layout, so 2 visible + 1 overflow
        assert_eq!(all_visible.len(), 2, "2-col layout shows 2 visible windows");

        let all_windows = mgr.active_workspace().layout.windows();
        assert_eq!(all_windows.len(), 3, "3 windows total in layout");
    }

    // -----------------------------------------------------------------------
    // Scenario 3: "Overflow management"
    // -----------------------------------------------------------------------
    #[test]
    fn test_workflow_overflow_management() {
        let mut layouts = HashMap::new();
        layouts.insert(1, two_col_layout());
        let mut mgr = manager_with(layouts);

        let win1 = wid(1);
        let win2 = wid(2);
        let win3 = wid(3);
        let win4 = wid(4);

        // 1. 2-col layout, add 4 windows
        mgr.add_window(win1); // zone 0
        mgr.add_window(win2); // zone 1
        mgr.add_window(win3); // overflow -> last fill_order zone (zone 1)
        mgr.add_window(win4); // overflow -> last fill_order zone (zone 1)

        let positions = mgr.recalculate_active();

        // Zone 0: win1 (visible). Zone 1: win2 (visible), win3 + win4 (overflow/offscreen)
        let rect1 = find_rect(&positions.moves, win1).unwrap();
        let rect2 = find_rect(&positions.moves, win2).unwrap();
        let rect3 = find_rect(&positions.moves, win3).unwrap();
        let rect4 = find_rect(&positions.moves, win4).unwrap();

        assert!(!is_offscreen(&rect1), "win1 should be visible");
        assert!(!is_offscreen(&rect2), "win2 should be visible");
        assert!(is_offscreen(&rect3), "win3 should be overflow/offscreen");
        assert!(is_offscreen(&rect4), "win4 should be overflow/offscreen");

        // 2. Focus direction down on zone with overflow -> cycles to hidden overflow window
        //    Focus is on zone 1 (last fill_order slot). Zone 1 = [win2, win3, win4].
        //    focused() returns win2 (index 0 = visible window).
        assert_eq!(mgr.active_workspace().layout.focused(), Some(win2));
        // Down at zone boundary cycles overflow via focus_next.
        // focus_next rotates left: [win2, win3, win4] -> [win3, win4, win2]
        mgr.focus_direction(Direction::Down);
        assert_eq!(
            mgr.active_workspace().layout.focused(),
            Some(win3),
            "cycling overflow should bring win3 to front"
        );

        // 3. Focus direction down again -> cycles again: [win3, win4, win2] -> [win4, win2, win3]
        mgr.focus_direction(Direction::Down);
        assert_eq!(
            mgr.active_workspace().layout.focused(),
            Some(win4),
            "cycling overflow again should bring win4 to front"
        );

        // 4. Remove visible window from zone with overflow -> overflow promotes to visible
        //    Currently focused on zone 1 with win4 at front: [win4, win2, win3]
        let visible_in_zone1 = mgr.active_workspace().layout.focused().unwrap();
        assert_eq!(visible_in_zone1, win4);

        // Remove the visible window from zone 1
        mgr.remove_window(visible_in_zone1);

        // 5. Verify positions: promoted window gets the zone rect, not offscreen
        let positions = mgr.recalculate_active();
        // Zone 1 should still have windows (promoted from overflow)
        let zone1_windows: Vec<_> = positions
            .moves
            .iter()
            .filter(|(_, r)| !is_offscreen(r) && r.x > 900.0)
            .collect();
        assert!(
            !zone1_windows.is_empty(),
            "after removing visible window, overflow should promote to visible"
        );
    }

    // -----------------------------------------------------------------------
    // Scenario 4: "Workspace switch preserves zone state"
    // -----------------------------------------------------------------------
    #[test]
    fn test_workflow_workspace_switch_preserves_state() {
        let mut layouts = HashMap::new();
        layouts.insert(1, two_col_layout());
        // ws2 stays monocle
        let mut mgr = manager_with(layouts);

        let win1 = wid(1);
        let win2 = wid(2);
        let win3 = wid(3);
        let win4 = wid(4);

        // 1. ws1=2-col with 3 windows (one overflow)
        mgr.add_window(win1); // zone 0
        mgr.add_window(win2); // zone 1
        mgr.add_window(win3); // overflow to zone 1

        // Focus zone 0 for deterministic state
        mgr.focus_direction(Direction::Left);
        assert_eq!(mgr.active_workspace().layout.focused(), Some(win1));

        // Record pre-switch positions
        let positions_before = mgr.recalculate_active();
        let rect1_before = find_rect(&positions_before.moves, win1).unwrap();
        let rect2_before = find_rect(&positions_before.moves, win2).unwrap();
        let rect3_before = find_rect(&positions_before.moves, win3).unwrap();

        // 2. Switch to ws2 -> add a window there
        mgr.switch_workspace(2);
        mgr.add_window(win4);
        assert_eq!(mgr.active_index(), 1);

        // All ws1 windows should be offscreen now (they were moved offscreen by switch)
        // ws2 window should be visible
        let ws2_positions = mgr.recalculate_active();
        let rect4 = find_rect(&ws2_positions.moves, win4).unwrap();
        assert!(!is_offscreen(&rect4), "ws2 window should be visible");

        // 3. Switch back to ws1 -> zone layout restored
        let transition = mgr.switch_workspace(1);
        assert_eq!(mgr.active_index(), 0);

        // ws1 zone windows should be back on-screen
        let onscreen: Vec<_> = transition
            .moves
            .iter()
            .filter(|(_, r)| !is_offscreen(r))
            .collect();
        assert_eq!(onscreen.len(), 2, "2 visible zone windows should be on-screen after switch back");

        // ws2 window should go offscreen
        let offscreen_ws2: Vec<_> = transition
            .moves
            .iter()
            .filter(|(id, r)| *id == win4 && is_offscreen(r))
            .collect();
        assert_eq!(offscreen_ws2.len(), 1, "ws2 window should go offscreen");

        // 4. Verify exact pixel positions match pre-switch state
        let positions_after = mgr.recalculate_active();
        let rect1_after = find_rect(&positions_after.moves, win1).unwrap();
        let rect2_after = find_rect(&positions_after.moves, win2).unwrap();
        let rect3_after = find_rect(&positions_after.moves, win3).unwrap();

        assert_eq!(rect1_before, rect1_after, "win1 position should be restored exactly");
        assert_eq!(rect2_before, rect2_after, "win2 position should be restored exactly");
        assert_eq!(rect3_before, rect3_after, "win3 overflow position should be restored exactly");

        // Focused window should still be win1 (zone 0)
        assert_eq!(
            mgr.active_workspace().layout.focused(),
            Some(win1),
            "focused zone should be preserved across workspace switch"
        );
    }

    // -----------------------------------------------------------------------
    // Scenario 5: "Config-driven layout construction"
    // -----------------------------------------------------------------------
    #[test]
    fn test_workflow_config_driven_layout() {
        // 1. Parse a realistic TOML config with a composite main-stack layout
        let toml_str = r#"
[gaps]
inner = 8.0
outer = 10.0

[layouts.main-stack]
type = "columns"
ratios = [0.6, 0.4]

[layouts.main-stack.splits.1]
type = "rows"
ratios = [0.5, 0.5]

[workspaces.1]
layout = "main-stack"
"#;
        let config = Config::from_toml(toml_str).unwrap();

        // 2. Call build_workspace_layouts()
        let layouts = config.build_workspace_layouts();
        assert!(layouts.contains_key(&1));

        // 3. Create WorkspaceManager with those layouts
        let mut mgr = WorkspaceManager::new_with_layouts(layouts);
        mgr.set_screen_and_gaps(screen(), gaps());

        // 4. Add windows, verify the composite layout produces 3 leaf zones
        let win_main = wid(1);
        let win_top = wid(2);
        let win_bot = wid(3);

        mgr.add_window(win_main); // fill_order[0] = zone 0 (main, left column)
        mgr.add_window(win_top);  // fill_order[1] = zone 1 (top-right)
        mgr.add_window(win_bot);  // fill_order[2] = zone 2 (bottom-right)

        let positions = mgr.recalculate_active();
        assert_eq!(positions.moves.len(), 3, "3 leaf zones = 3 windows visible");

        let rect_main = find_rect(&positions.moves, win_main).unwrap();
        let rect_top = find_rect(&positions.moves, win_top).unwrap();
        let rect_bot = find_rect(&positions.moves, win_bot).unwrap();

        // All should be on-screen
        assert!(!is_offscreen(&rect_main));
        assert!(!is_offscreen(&rect_top));
        assert!(!is_offscreen(&rect_bot));

        // 5. Verify rects: main column ~60% width, right column ~40% width split into 2 rows
        // Inner screen: x=10, y=10, width=1900, height=1060 (after outer gaps)
        // Usable width for HSplit: 1900 - 8 (1 inner gap) = 1892
        // Main column: 1892 * 0.6 = 1135.2
        // Right column: 1892 * 0.4 = 756.8
        let expected_main_width = (1900.0 - 8.0) * 0.6;
        assert!(
            (rect_main.width - expected_main_width).abs() < 1.0,
            "main column should be ~60% width, got {} expected {}",
            rect_main.width,
            expected_main_width
        );

        let expected_right_width = (1900.0 - 8.0) * 0.4;
        assert!(
            (rect_top.width - expected_right_width).abs() < 1.0,
            "right column should be ~40% width, got {} expected {}",
            rect_top.width,
            expected_right_width
        );

        // Top and bottom right should have same width
        assert!(
            (rect_top.width - rect_bot.width).abs() < 0.01,
            "top and bottom right should have same width"
        );

        // Top and bottom right should split the height (50/50 minus inner gap)
        // Right column height: 1060 (inner screen height)
        // Usable height for VSplit: 1060 - 8 = 1052
        // Each row: 1052 * 0.5 = 526
        let expected_row_height = (1060.0 - 8.0) * 0.5;
        assert!(
            (rect_top.height - expected_row_height).abs() < 1.0,
            "top-right row height should be ~50%, got {} expected {}",
            rect_top.height,
            expected_row_height
        );
        assert!(
            (rect_bot.height - expected_row_height).abs() < 1.0,
            "bottom-right row height should be ~50%, got {} expected {}",
            rect_bot.height,
            expected_row_height
        );

        // Bottom-right should be below top-right
        assert!(
            rect_bot.y > rect_top.y,
            "bottom-right zone should be below top-right"
        );

        // 6. Navigate between all 3 zones with focus_direction
        // Start at win_bot (last added)
        assert_eq!(mgr.active_workspace().layout.focused(), Some(win_bot));

        // Focus up -> should go to top-right (zone 1)
        mgr.focus_direction(Direction::Up);
        assert_eq!(mgr.active_workspace().layout.focused(), Some(win_top));

        // Focus left -> should go to main (zone 0)
        mgr.focus_direction(Direction::Left);
        assert_eq!(mgr.active_workspace().layout.focused(), Some(win_main));

        // Focus right -> should go to top-right or bottom-right (adjacent zone)
        mgr.focus_direction(Direction::Right);
        let focused = mgr.active_workspace().layout.focused().unwrap();
        assert!(
            focused == win_top || focused == win_bot,
            "focus right from main should go to right column"
        );
    }

    // -----------------------------------------------------------------------
    // Scenario 6: "Graceful shutdown restores all windows"
    // -----------------------------------------------------------------------
    #[test]
    fn test_workflow_graceful_shutdown() {
        // 1. Create mixed layout manager
        let mut layouts = HashMap::new();
        layouts.insert(1, two_col_layout());      // ws1: zone with windows
        // ws2: default monocle
        layouts.insert(3, two_col_layout());       // ws3: zone with windows
        let mut mgr = manager_with(layouts);

        // ws1: 3 windows (2 in zones + 1 overflow)
        mgr.add_window(wid(1)); // zone 0
        mgr.add_window(wid(2)); // zone 1
        mgr.add_window(wid(3)); // overflow

        // ws2: 1 monocle window
        mgr.switch_workspace(2);
        mgr.add_window(wid(4));

        // ws3: 2 zone windows
        mgr.switch_workspace(3);
        mgr.add_window(wid(5)); // zone 0
        mgr.add_window(wid(6)); // zone 1

        // 2. Float 1 window on ws1
        mgr.switch_workspace(1);
        // Focus win1 in zone 0 and float it
        mgr.focus_direction(Direction::Left);
        mgr.toggle_float(wid(1));

        // 3. Call get_all_window_positions()
        let positions = mgr.get_all_window_positions();

        // 4. Verify ALL 6 windows get on-screen positions (none offscreen)
        assert_eq!(positions.len(), 6, "all 6 windows should have positions");
        for (id, rect) in &positions {
            assert!(
                !is_offscreen(rect),
                "window {:?} should be on-screen during shutdown, got x={}",
                id,
                rect.x
            );
        }

        // 5. Verify no window overlaps the menu bar area (y >= outer_gap)
        for (id, rect) in &positions {
            assert!(
                rect.y >= gaps().outer,
                "window {:?} y={} should be >= outer_gap={} (menu bar clearance)",
                id,
                rect.y,
                gaps().outer
            );
        }
    }

    // -----------------------------------------------------------------------
    // Scenario 7: "Edge case - empty zones and single window"
    // -----------------------------------------------------------------------
    #[test]
    fn test_workflow_empty_zones_single_window() {
        // 1. 3-col layout, add only 1 window -> goes to center (fill_order[0] = 1)
        let mut layouts = HashMap::new();
        layouts.insert(1, three_col_layout([0.30, 0.40, 0.30]));
        let mut mgr = manager_with(layouts);

        let win1 = wid(1);
        mgr.add_window(win1);

        // 2. Verify: center zone has the window rect, other zones are empty
        let positions = mgr.recalculate_active();
        let visible: Vec<_> = positions
            .moves
            .iter()
            .filter(|(_, r)| !is_offscreen(r))
            .collect();
        assert_eq!(visible.len(), 1, "only 1 window should be visible");

        let rect1 = find_rect(&positions.moves, win1).unwrap();
        assert!(!is_offscreen(&rect1), "single window should be visible");

        // The window should be in the center zone -- verify x position is between left and right
        // Inner screen: x=10, width=1900. Left zone: 30% of usable, center: 40%, right: 30%
        // Center zone x should be roughly at 10 + (1900 - 2*8) * 0.30 + 8 = ~574.6
        assert!(
            rect1.x > 100.0,
            "center zone should not start at the left edge, got x={}",
            rect1.x
        );

        // 3. Focus left -> no-op (empty zone)
        mgr.focus_direction(Direction::Left);
        assert_eq!(
            mgr.active_workspace().layout.focused(),
            Some(win1),
            "focus left into empty zone should be no-op"
        );

        // 4. Focus right -> no-op (empty zone)
        mgr.focus_direction(Direction::Right);
        assert_eq!(
            mgr.active_workspace().layout.focused(),
            Some(win1),
            "focus right into empty zone should be no-op"
        );

        // 5. Add second window -> goes to left zone (fill_order[1] = 0)
        let win2 = wid(2);
        mgr.add_window(win2);

        let positions = mgr.recalculate_active();
        let visible: Vec<_> = positions
            .moves
            .iter()
            .filter(|(_, r)| !is_offscreen(r))
            .collect();
        assert_eq!(visible.len(), 2, "2 windows should now be visible");

        let rect2 = find_rect(&positions.moves, win2).unwrap();
        assert!(
            rect2.x < rect1.x,
            "second window should be in left zone (x={}) left of center (x={})",
            rect2.x,
            rect1.x
        );

        // 6. Now focus left works (zone 0 has a window)
        // Focus is on win2 (last added, zone 0/left). Focus right should go to center.
        mgr.focus_direction(Direction::Right);
        assert_eq!(
            mgr.active_workspace().layout.focused(),
            Some(win1),
            "focus right should now reach center zone with win1"
        );

        // And back to left
        mgr.focus_direction(Direction::Left);
        assert_eq!(
            mgr.active_workspace().layout.focused(),
            Some(win2),
            "focus left should go back to win2 in left zone"
        );
    }
}
