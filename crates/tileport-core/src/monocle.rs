use crate::types::WindowId;

/// Monocle layout -- one window visible at a time, carousel navigation.
///
/// Implementation is deferred to Phase 2. This module provides the struct
/// definition and empty method stubs for the workspace module to compile against.
#[derive(Debug)]
pub struct MonocleLayout {
    #[allow(dead_code)]
    windows: Vec<WindowId>,
    #[allow(dead_code)]
    focused_index: Option<usize>,
}

impl MonocleLayout {
    /// Create an empty monocle layout.
    pub fn new() -> Self {
        Self {
            windows: Vec::new(),
            focused_index: None,
        }
    }

    /// Number of windows in the layout.
    pub fn len(&self) -> usize {
        self.windows.len()
    }

    /// Whether the layout has no windows.
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }
}

impl Default for MonocleLayout {
    fn default() -> Self {
        Self::new()
    }
}
