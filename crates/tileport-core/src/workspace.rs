use crate::monocle::MonocleLayout;
use crate::types::WindowId;

/// A workspace holds a monocle layout and a set of floating windows.
///
/// Implementation is deferred to Phase 2. This module provides the struct
/// definitions for the crate's public API surface.
#[derive(Debug)]
pub struct Workspace {
    /// Workspace number (1-9).
    pub id: u8,
    /// The monocle layout for tiled windows.
    pub monocle: MonocleLayout,
    /// Windows that have been floated out of the monocle layout.
    pub floating_windows: Vec<WindowId>,
    /// The currently focused window (may be tiled or floating).
    pub focused_window: Option<WindowId>,
}

impl Workspace {
    /// Create a new empty workspace.
    pub fn new(id: u8) -> Self {
        Self {
            id,
            monocle: MonocleLayout::new(),
            floating_windows: Vec::new(),
            focused_window: None,
        }
    }
}

/// Manages all 9 workspaces and tracks the active one.
///
/// Full implementation deferred to Phase 2.
#[derive(Debug)]
pub struct WorkspaceManager {
    #[allow(dead_code)]
    workspaces: [Workspace; 9],
    #[allow(dead_code)]
    active_index: usize,
}

impl WorkspaceManager {
    /// Create a workspace manager with 9 empty workspaces. Workspace 1 is active.
    pub fn new() -> Self {
        Self {
            workspaces: std::array::from_fn(|i| Workspace::new((i + 1) as u8)),
            active_index: 0,
        }
    }
}

impl Default for WorkspaceManager {
    fn default() -> Self {
        Self::new()
    }
}
