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
