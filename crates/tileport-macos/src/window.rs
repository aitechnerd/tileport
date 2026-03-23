//! Window enumeration, move/resize, and observation.
//!
//! Uses CGWindowListCopyWindowInfo to discover window PIDs, then creates
//! AXUIElement wrappers per app to get manageable windows.

use crate::accessibility::{AXApp, AXWindow};
use anyhow::Result;
use core_foundation::base::TCFType;
use core_foundation::number::CFNumber;
use core_graphics::window::{
    copy_window_info, kCGNullWindowID, kCGWindowListOptionOnScreenOnly,
    kCGWindowListExcludeDesktopElements, kCGWindowOwnerPID,
};
use std::collections::HashSet;
use tileport_core::platform::WindowInfo;
use tileport_core::types::Rect;

/// Enumerate all standard (manageable) windows on screen.
///
/// Uses CGWindowListCopyWindowInfo to get the PID list, then queries each
/// app's AX element for its windows. Filters out non-standard windows
/// (menus, status items, Dock, desktop elements).
pub fn enumerate_windows() -> Result<Vec<(WindowInfo, AXWindow)>> {
    let options = kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements;
    let window_list = copy_window_info(options, kCGNullWindowID)
        .ok_or_else(|| anyhow::anyhow!("CGWindowListCopyWindowInfo returned null"))?;

    // Collect unique PIDs from the window list.
    let mut pids = HashSet::new();
    for i in 0..window_list.len() {
        // SAFETY: Each element in the array is a CFDictionaryRef.
        let dict_ref = unsafe {
            core_foundation::array::CFArrayGetValueAtIndex(
                window_list.as_concrete_TypeRef(),
                i,
            ) as core_foundation::dictionary::CFDictionaryRef
        };

        if let Some(pid_value) = get_dict_pid(dict_ref) {
            pids.insert(pid_value as i32);
        }
    }

    let mut results = Vec::new();

    for pid in pids {
        let app = AXApp::new(pid);
        let ax_windows = match app.windows() {
            Ok(w) => w,
            Err(e) => {
                tracing::debug!(pid, error = %e, "failed to get windows for app");
                continue;
            }
        };

        let app_id = get_app_bundle_id(pid).unwrap_or_default();

        for ax_window in ax_windows {
            if !ax_window.is_standard_window() {
                continue;
            }

            let wid = match ax_window.window_id() {
                Ok(id) => id,
                Err(e) => {
                    tracing::debug!(pid, error = %e, "failed to get window ID");
                    continue;
                }
            };

            let title = ax_window.get_title().unwrap_or_default();

            // Skip windows with zero size (invisible helper windows).
            if let Ok((w, h)) = ax_window.get_size() {
                if w <= 0.0 || h <= 0.0 {
                    continue;
                }
            }

            let info = WindowInfo {
                window_id: wid,
                app_id: app_id.clone(),
                title,
                pid,
            };

            results.push((info, ax_window));
        }
    }

    Ok(results)
}

/// Move and resize a window to the given rectangle.
///
/// Sets position first, then size, to avoid visual artifacts where the
/// window briefly appears at the wrong size in the wrong position.
pub fn move_window(ax_window: &AXWindow, rect: Rect) -> Result<()> {
    ax_window.set_position(rect.x, rect.y)?;
    ax_window.set_size(rect.width, rect.height)?;
    Ok(())
}

/// Focus (raise) a window by setting its AXMain attribute and raising the app.
pub fn focus_window(ax_window: &AXWindow) -> Result<()> {
    use accessibility_sys::*;
    use core_foundation::base::CFTypeRef;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::string::CFString;

    // Set AXMain = true to make it the main window.
    let attr = CFString::from_static_string(kAXMainAttribute);
    let value = CFBoolean::true_value();
    // SAFETY: Setting AXMain to true raises the window within its app.
    let err = unsafe {
        AXUIElementSetAttributeValue(
            ax_window.as_raw(),
            attr.as_concrete_TypeRef(),
            value.as_CFTypeRef() as CFTypeRef,
        )
    };
    if err != kAXErrorSuccess {
        tracing::warn!(error = err, "failed to set AXMain on window");
    }

    // Also perform the AXRaise action to bring it to front.
    let raise = CFString::new("AXRaise");
    // SAFETY: AXUIElementPerformAction is a public AX API.
    let err = unsafe {
        AXUIElementPerformAction(ax_window.as_raw(), raise.as_concrete_TypeRef())
    };
    if err != kAXErrorSuccess {
        tracing::warn!(error = err, "failed to perform AXRaise on window");
    }

    Ok(())
}

/// Raise the application that owns the given PID to the front.
pub fn raise_app(pid: i32) -> Result<()> {
    use accessibility_sys::*;
    use core_foundation::base::CFTypeRef;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::string::CFString;

    let app = AXApp::new(pid);
    let attr = CFString::from_static_string(kAXFrontmostAttribute);
    let value = CFBoolean::true_value();
    // SAFETY: Setting AXFrontmost to true makes the app frontmost.
    let err = unsafe {
        AXUIElementSetAttributeValue(
            app.as_raw(),
            attr.as_concrete_TypeRef(),
            value.as_CFTypeRef() as CFTypeRef,
        )
    };
    if err != kAXErrorSuccess {
        tracing::warn!(pid, error = err, "failed to set app frontmost");
    }
    Ok(())
}

// -- Helpers --

/// Get a number value from a CFDictionary using the raw kCGWindowOwnerPID key.
fn get_dict_pid(dict_ref: core_foundation::dictionary::CFDictionaryRef) -> Option<i64> {
    // SAFETY: We use the system-provided kCGWindowOwnerPID key to look up
    // the PID in the window info dictionary. The value is a CFNumber.
    unsafe {
        let mut pid_ref: core_foundation::base::CFTypeRef = std::ptr::null();
        if core_foundation::dictionary::CFDictionaryGetValueIfPresent(
            dict_ref,
            kCGWindowOwnerPID as *const _,
            &mut pid_ref,
        ) != 0
            && !pid_ref.is_null()
        {
            let num =
                CFNumber::wrap_under_get_rule(pid_ref as core_foundation::number::CFNumberRef);
            num.to_i64()
        } else {
            None
        }
    }
}

/// Get the bundle identifier for a running application by PID.
fn get_app_bundle_id(pid: i32) -> Option<String> {
    use objc2_app_kit::NSRunningApplication;

    // runningApplicationWithProcessIdentifier: is safe from any thread.
    let app = NSRunningApplication::runningApplicationWithProcessIdentifier(pid)?;
    let bundle_id = app.bundleIdentifier();
    bundle_id.map(|id| id.to_string())
}
