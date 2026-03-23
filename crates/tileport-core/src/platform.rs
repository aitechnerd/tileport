use crate::types::{Rect, WindowId};

/// Information about a discovered window.
#[derive(Debug, Clone)]
pub struct WindowInfo {
    /// macOS CGWindowID.
    pub window_id: WindowId,
    /// Bundle identifier of the owning application (e.g. "com.apple.Safari").
    pub app_id: String,
    /// Window title.
    pub title: String,
    /// Process ID of the owning application.
    pub pid: i32,
}

/// Platform abstraction layer for window management operations.
///
/// Implemented by `MacOSPlatform` in tileport-macos. A mock implementation
/// can be used in manager tests (Phase 4).
pub trait PlatformApi: Send {
    /// Discover all standard (manageable) windows on screen.
    fn enumerate_windows(&self) -> anyhow::Result<Vec<WindowInfo>>;

    /// Move and resize a window to the given rectangle.
    fn move_window(&self, window_id: WindowId, rect: Rect) -> anyhow::Result<()>;

    /// Get the usable display bounds (excluding menu bar and Dock).
    fn get_display_bounds(&self) -> anyhow::Result<Rect>;

    /// Focus (raise) a window by its ID.
    fn focus_window(&self, window_id: WindowId) -> anyhow::Result<()>;
}
