//! Permission checks for Accessibility and Input Monitoring.
//!
//! macOS requires explicit user approval for apps to use the Accessibility
//! API (window manipulation) and Input Monitoring (global keyboard events).

use anyhow::{bail, Result};
use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::string::CFString;

/// Check whether the current process has Accessibility permission.
///
/// If not trusted, prompts the user via the macOS system dialog.
pub fn check_accessibility() -> bool {
    // SAFETY: AXIsProcessTrustedWithOptions is a public ApplicationServices API.
    // We create a dictionary with kAXTrustedCheckOptionPrompt = true so macOS
    // shows the permission dialog if the app is not yet trusted.
    unsafe {
        let key = CFString::wrap_under_get_rule(
            accessibility_sys::kAXTrustedCheckOptionPrompt,
        );
        let value = CFBoolean::true_value();
        let options = core_foundation::dictionary::CFDictionary::from_CFType_pairs(
            &[(key.as_CFType(), value.as_CFType())],
        );
        accessibility_sys::AXIsProcessTrustedWithOptions(
            options.as_concrete_TypeRef(),
        )
    }
}

/// Check whether the current process has Input Monitoring permission.
///
/// Uses `CGPreflightListenEventAccess` which returns true if the app can
/// create event taps for listening to keyboard events.
pub fn check_input_monitoring() -> bool {
    // SAFETY: CGPreflightListenEventAccess is a public CoreGraphics API
    // available on macOS 10.15+. It returns a bool indicating whether the
    // app has Listen Event access.
    unsafe { CGPreflightListenEventAccess() }
}

/// Ensure both Accessibility and Input Monitoring permissions are granted.
///
/// Prints clear instructions and returns an error if either is missing,
/// allowing the caller to halt startup (AC-16).
pub fn ensure_permissions() -> Result<()> {
    let has_accessibility = check_accessibility();
    let has_input_monitoring = check_input_monitoring();

    if !has_accessibility {
        eprintln!(
            "\ntileport requires Accessibility permission.\n\
             Open System Settings > Privacy & Security > Accessibility\n\
             and add tileport-wm.\n"
        );
    }

    if !has_input_monitoring {
        eprintln!(
            "\ntileport requires Input Monitoring permission.\n\
             Open System Settings > Privacy & Security > Input Monitoring\n\
             and add tileport-wm.\n"
        );
    }

    if !has_accessibility || !has_input_monitoring {
        bail!(
            "missing permissions: accessibility={}, input_monitoring={}",
            has_accessibility,
            has_input_monitoring
        );
    }

    Ok(())
}

// CGPreflightListenEventAccess is not exposed by the core-graphics crate.
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGPreflightListenEventAccess() -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_accessibility_returns_bool() {
        // In CI or without permissions this returns false; that's fine.
        // We just verify it doesn't crash.
        let _result = check_accessibility();
    }

    #[test]
    fn check_input_monitoring_returns_bool() {
        let _result = check_input_monitoring();
    }
}
