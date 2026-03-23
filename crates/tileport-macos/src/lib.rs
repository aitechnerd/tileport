//! tileport-macos: macOS platform bindings.
//!
//! This crate isolates all macOS-specific FFI code (Accessibility API,
//! Core Graphics, AppKit). All unsafe code is confined here with proper
//! safety comments and resource management (CFRetain/CFRelease).

pub mod accessibility;
pub mod display;
pub mod hotkey;
pub mod permission;
pub mod window;

use std::collections::HashMap;

use crate::accessibility::AXWindow;
use tileport_core::platform::{PlatformApi, WindowInfo};
use tileport_core::types::{Rect, WindowId};

/// macOS implementation of the PlatformApi trait.
///
/// Holds a map of WindowId -> AXWindow references for efficient lookup
/// when moving/focusing windows. The map is updated as windows are
/// discovered, created, or destroyed.
pub struct MacOSPlatform {
    /// Map from WindowId to the AX element reference for that window.
    /// Entries are added during enumeration and window-created events,
    /// and removed during window-destroyed events.
    windows: HashMap<WindowId, AXWindow>,
}

// SAFETY: MacOSPlatform is Send because AXWindow is Send (we marked it so)
// and HashMap is Send when both K and V are Send.
// The manager thread owns this exclusively -- no sharing between threads.
unsafe impl Send for MacOSPlatform {}

impl MacOSPlatform {
    /// Create a new MacOSPlatform with no tracked windows.
    pub fn new() -> Self {
        Self {
            windows: HashMap::new(),
        }
    }

    /// Register an AXWindow for a given WindowId.
    ///
    /// Called during initial enumeration and when new windows are detected.
    pub fn register_window(&mut self, window_id: WindowId, ax_window: AXWindow) {
        self.windows.insert(window_id, ax_window);
    }

    /// Unregister a window (e.g., when it's destroyed).
    pub fn unregister_window(&mut self, window_id: WindowId) {
        self.windows.remove(&window_id);
    }

    /// Get an AXWindow reference by WindowId.
    pub fn get_ax_window(&self, window_id: WindowId) -> Option<&AXWindow> {
        self.windows.get(&window_id)
    }
}

impl Default for MacOSPlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformApi for MacOSPlatform {
    fn enumerate_windows(&self) -> anyhow::Result<Vec<WindowInfo>> {
        let results = window::enumerate_windows()?;
        Ok(results.into_iter().map(|(info, _ax)| info).collect())
    }

    fn move_window(&self, window_id: WindowId, rect: Rect) -> anyhow::Result<()> {
        let ax_window = self.windows.get(&window_id).ok_or_else(|| {
            anyhow::anyhow!("no AX element for window {:?}", window_id)
        })?;
        window::move_window(ax_window, rect)
    }

    fn get_display_bounds(&self) -> anyhow::Result<Rect> {
        display::get_primary_display()
    }

    fn focus_window(&self, window_id: WindowId) -> anyhow::Result<()> {
        let ax_window = self.windows.get(&window_id).ok_or_else(|| {
            anyhow::anyhow!("no AX element for window {:?}", window_id)
        })?;
        window::focus_window(ax_window)
    }
}
