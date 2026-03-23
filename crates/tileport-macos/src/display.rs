//! Screen/display bounds queries.
//!
//! Returns the usable display area (excluding menu bar and Dock) for the
//! primary monitor. Multi-monitor support is deferred to V2.

use anyhow::Result;
use core_graphics::display::CGDisplay;
use tileport_core::types::Rect;

/// Get the usable bounds of the primary display.
///
/// Uses NSScreen.visibleFrame via CGDisplay as fallback. The visible frame
/// excludes the menu bar and Dock area, giving us the area where windows
/// can actually be placed.
pub fn get_primary_display() -> Result<Rect> {
    // Try NSScreen first (requires main thread marker in objc2).
    // Fall back to CGDisplay which works from any thread.
    get_display_bounds_cg()
}

/// Get display bounds using CoreGraphics (works from any thread).
///
/// CGDisplayBounds returns the full display rect. We then query the
/// NSScreen visibleFrame for the usable area. Since NSScreen requires
/// MainThreadMarker in objc2, we use a CGDisplay-based approach with
/// the known menu bar height as approximation, then try NSScreen if
/// we can get a MainThreadMarker.
fn get_display_bounds_cg() -> Result<Rect> {
    let display = CGDisplay::main();
    let bounds = display.bounds();

    // Try to get the actual visible frame via NSScreen.
    if let Some(visible) = get_nsscreen_visible_frame() {
        return Ok(visible);
    }

    // Fallback: use full display bounds minus estimated menu bar (24px).
    // This is only used if NSScreen is unavailable (shouldn't happen on macOS).
    let menu_bar_height = 25.0;
    Ok(Rect {
        x: bounds.origin.x,
        y: bounds.origin.y + menu_bar_height,
        width: bounds.size.width,
        height: bounds.size.height - menu_bar_height,
    })
}

/// Attempt to get the visible frame from NSScreen.
///
/// Returns None if we can't obtain a MainThreadMarker (e.g., called from
/// a non-main thread without NSApplication running).
fn get_nsscreen_visible_frame() -> Option<Rect> {
    use objc2_app_kit::NSScreen;
    use objc2_foundation::MainThreadMarker;

    // MainThreadMarker::new() returns Some only if called from the main thread.
    let mtm = MainThreadMarker::new()?;
    let screen = NSScreen::mainScreen(mtm)?;
    let frame = screen.visibleFrame();

    // NSScreen coordinates have origin at bottom-left. We need to convert
    // to top-left origin (which is what CGDisplay/AX API uses).
    let full_frame = screen.frame();
    let y = full_frame.size.height - frame.origin.y - frame.size.height;

    Some(Rect {
        x: frame.origin.x,
        y,
        width: frame.size.width,
        height: frame.size.height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_primary_display_returns_nonzero() {
        // This test may fail in headless CI but should pass on macOS with a display.
        if let Ok(rect) = get_primary_display() {
            assert!(rect.width > 0.0, "display width should be positive");
            assert!(rect.height > 0.0, "display height should be positive");
        }
        // If it errors (no display), that's acceptable in CI.
    }
}
