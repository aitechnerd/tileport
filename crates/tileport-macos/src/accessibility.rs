//! Safe wrappers around AXUIElement for window manipulation.
//!
//! AXUIElementRef is a Core Foundation type that must be retained on clone
//! and released on drop (DevSecOps requirement: no CF object leaks).

use accessibility_sys::*;
use anyhow::{anyhow, bail, Result};
use core_foundation::base::{CFRelease, CFRetain, CFTypeRef, TCFType};
use core_foundation::string::CFString;
use core_graphics::geometry::{CGPoint, CGSize};
use tileport_core::types::WindowId;

/// Safe wrapper around an AXUIElementRef representing a window.
///
/// Implements Clone via CFRetain and Drop via CFRelease to ensure proper
/// Core Foundation reference counting.
pub struct AXWindow {
    element: AXUIElementRef,
}

// SAFETY: AXUIElementRef is a CF type that is safe to send between threads
// as long as we maintain proper retain/release semantics, which we do.
// The manager thread owns all AXWindow instances exclusively.
unsafe impl Send for AXWindow {}

impl AXWindow {
    /// Create a new AXWindow from a raw AXUIElementRef.
    ///
    /// # Safety
    /// The caller must ensure `element` is a valid, retained AXUIElementRef.
    /// This struct takes ownership of the reference (will CFRelease on drop).
    pub unsafe fn from_raw(element: AXUIElementRef) -> Self {
        debug_assert!(!element.is_null());
        Self { element }
    }

    /// Create a new AXWindow from a raw AXUIElementRef, retaining it.
    ///
    /// # Safety
    /// The caller must ensure `element` is a valid AXUIElementRef.
    pub unsafe fn from_raw_retained(element: AXUIElementRef) -> Self {
        debug_assert!(!element.is_null());
        // SAFETY: We retain the element so the caller's reference stays valid.
        unsafe { CFRetain(element as CFTypeRef) };
        Self { element }
    }

    /// Get the raw element ref (does NOT transfer ownership).
    pub fn as_raw(&self) -> AXUIElementRef {
        self.element
    }

    /// Get the window position (top-left corner).
    pub fn get_position(&self) -> Result<(f64, f64)> {
        let value = self.get_attribute_value(kAXPositionAttribute)?;
        let mut point = CGPoint::new(0.0, 0.0);
        // SAFETY: We verified the attribute value is a position type via AX API.
        let ok = unsafe {
            AXValueGetValue(
                value as AXValueRef,
                kAXValueTypeCGPoint,
                &mut point as *mut CGPoint as *mut std::ffi::c_void,
            )
        };
        if !ok {
            bail!("failed to extract CGPoint from AXValue");
        }
        // SAFETY: We own this CF value reference; release it.
        unsafe { CFRelease(value) };
        Ok((point.x, point.y))
    }

    /// Set the window position (top-left corner).
    pub fn set_position(&self, x: f64, y: f64) -> Result<()> {
        let point = CGPoint::new(x, y);
        // SAFETY: AXValueCreate returns a new CF object; we own it.
        let value = unsafe {
            AXValueCreate(
                kAXValueTypeCGPoint,
                &point as *const CGPoint as *const std::ffi::c_void,
            )
        };
        if value.is_null() {
            bail!("failed to create AXValue for position");
        }
        let result = self.set_attribute_value(kAXPositionAttribute, value as CFTypeRef);
        // SAFETY: We created this value; release it.
        unsafe { CFRelease(value as CFTypeRef) };
        result
    }

    /// Get the window size.
    pub fn get_size(&self) -> Result<(f64, f64)> {
        let value = self.get_attribute_value(kAXSizeAttribute)?;
        let mut size = CGSize::new(0.0, 0.0);
        // SAFETY: We verified the attribute value is a size type via AX API.
        let ok = unsafe {
            AXValueGetValue(
                value as AXValueRef,
                kAXValueTypeCGSize,
                &mut size as *mut CGSize as *mut std::ffi::c_void,
            )
        };
        if !ok {
            bail!("failed to extract CGSize from AXValue");
        }
        // SAFETY: Release the CF value reference.
        unsafe { CFRelease(value) };
        Ok((size.width, size.height))
    }

    /// Set the window size.
    pub fn set_size(&self, width: f64, height: f64) -> Result<()> {
        let size = CGSize::new(width, height);
        // SAFETY: AXValueCreate returns a new CF object.
        let value = unsafe {
            AXValueCreate(
                kAXValueTypeCGSize,
                &size as *const CGSize as *const std::ffi::c_void,
            )
        };
        if value.is_null() {
            bail!("failed to create AXValue for size");
        }
        let result = self.set_attribute_value(kAXSizeAttribute, value as CFTypeRef);
        // SAFETY: Release our created value.
        unsafe { CFRelease(value as CFTypeRef) };
        result
    }

    /// Get the window title.
    pub fn get_title(&self) -> Result<String> {
        self.get_string_attribute(kAXTitleAttribute)
    }

    /// Get the AX role (e.g., "AXWindow").
    pub fn get_role(&self) -> Result<String> {
        self.get_string_attribute(kAXRoleAttribute)
    }

    /// Get the AX subrole (e.g., "AXStandardWindow", "AXSystemDialog").
    pub fn get_subrole(&self) -> Result<String> {
        self.get_string_attribute(kAXSubroleAttribute)
    }

    /// Check if this is a standard, manageable window.
    ///
    /// Filters out menus, Dock items, desktop elements, and system dialogs.
    pub fn is_standard_window(&self) -> bool {
        let role = match self.get_role() {
            Ok(r) => r,
            Err(_) => return false,
        };
        if role != kAXWindowRole {
            return false;
        }

        // Check subrole: accept "AXStandardWindow" or missing subrole.
        // Reject "AXSystemDialog" (Dock), "AXDialog" can be acceptable.
        match self.get_subrole() {
            Ok(subrole) => {
                subrole == "AXStandardWindow"
                    || subrole == "AXDialog"
                    || subrole == "AXFloatingWindow"
            }
            // No subrole is acceptable for some apps.
            Err(_) => true,
        }
    }

    /// Get the CGWindowID for this AX element using the private API.
    ///
    /// Uses `_AXUIElementGetWindow` which has been available since macOS 10.10
    /// and is used by Aerospace, yabai, and other window managers.
    pub fn window_id(&self) -> Result<WindowId> {
        // SAFETY: _AXUIElementGetWindow is a private but stable API.
        // We verify the function is available via dlsym before calling it.
        // The function writes a CGWindowID (u32) to the output pointer.
        unsafe {
            let func = resolve_ax_get_window()?;
            let mut wid: u32 = 0;
            let err = func(self.element, &mut wid);
            if err != kAXErrorSuccess {
                bail!("_AXUIElementGetWindow failed with error {}", err);
            }
            Ok(WindowId(wid))
        }
    }

    // -- Internal helpers --

    fn get_attribute_value(&self, attribute: &str) -> Result<CFTypeRef> {
        let attr = CFString::new(attribute);
        let mut value: CFTypeRef = std::ptr::null();
        // SAFETY: AXUIElementCopyAttributeValue is a public AX API.
        // On success it writes a retained CFTypeRef to `value`.
        let err = unsafe {
            AXUIElementCopyAttributeValue(
                self.element,
                attr.as_concrete_TypeRef(),
                &mut value,
            )
        };
        if err != kAXErrorSuccess {
            bail!("AXUIElementCopyAttributeValue({}) failed: {}", attribute, err);
        }
        if value.is_null() {
            bail!("AXUIElementCopyAttributeValue({}) returned null", attribute);
        }
        Ok(value)
    }

    fn set_attribute_value(&self, attribute: &str, value: CFTypeRef) -> Result<()> {
        let attr = CFString::new(attribute);
        // SAFETY: AXUIElementSetAttributeValue is a public AX API.
        let err = unsafe {
            AXUIElementSetAttributeValue(
                self.element,
                attr.as_concrete_TypeRef(),
                value,
            )
        };
        if err != kAXErrorSuccess {
            bail!("AXUIElementSetAttributeValue({}) failed: {}", attribute, err);
        }
        Ok(())
    }

    fn get_string_attribute(&self, attribute: &str) -> Result<String> {
        let value = self.get_attribute_value(attribute)?;
        // SAFETY: AXUIElementCopyAttributeValue returns a +1 retained CFTypeRef
        // (Create rule). We use wrap_under_create_rule so the CFString takes
        // ownership and releases the retain on drop, preventing a leak.
        let cf_string = unsafe {
            CFString::wrap_under_create_rule(value as core_foundation::string::CFStringRef)
        };
        Ok(cf_string.to_string())
    }
}

impl Clone for AXWindow {
    fn clone(&self) -> Self {
        // SAFETY: CFRetain increments the reference count. The new AXWindow
        // will call CFRelease on drop, matching this retain.
        unsafe { CFRetain(self.element as CFTypeRef) };
        Self {
            element: self.element,
        }
    }
}

impl Drop for AXWindow {
    fn drop(&mut self) {
        if !self.element.is_null() {
            // SAFETY: Every AXWindow owns a retain on its element.
            // CFRelease decrements the reference count.
            unsafe { CFRelease(self.element as CFTypeRef) };
        }
    }
}

impl std::fmt::Debug for AXWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AXWindow")
            .field("element", &self.element)
            .finish()
    }
}

/// Safe wrapper around an AXUIElementRef representing an application.
pub struct AXApp {
    element: AXUIElementRef,
    pid: i32,
}

// SAFETY: Same rationale as AXWindow -- proper retain/release semantics.
unsafe impl Send for AXApp {}

impl AXApp {
    /// Create an AXApp for the given process ID.
    pub fn new(pid: i32) -> Self {
        // SAFETY: AXUIElementCreateApplication is a public API that returns
        // a retained AXUIElementRef.
        let element = unsafe { AXUIElementCreateApplication(pid) };
        Self { element, pid }
    }

    /// Get the PID of this application.
    pub fn pid(&self) -> i32 {
        self.pid
    }

    /// Get all windows for this application.
    pub fn windows(&self) -> Result<Vec<AXWindow>> {
        let attr = CFString::from_static_string(kAXWindowsAttribute);
        let mut value: CFTypeRef = std::ptr::null();
        // SAFETY: AXUIElementCopyAttributeValue returns a retained CFArrayRef
        // containing the app's window elements.
        let err = unsafe {
            AXUIElementCopyAttributeValue(
                self.element,
                attr.as_concrete_TypeRef(),
                &mut value,
            )
        };
        if err != kAXErrorSuccess {
            // App might not have windows or AX access might be denied.
            return Ok(Vec::new());
        }
        if value.is_null() {
            return Ok(Vec::new());
        }

        // SAFETY: The returned value is a CFArrayRef of AXUIElementRef items.
        let array = unsafe {
            core_foundation::array::CFArray::<*const std::ffi::c_void>::wrap_under_create_rule(
                value as core_foundation::array::CFArrayRef,
            )
        };

        let mut windows = Vec::new();
        for i in 0..array.len() {
            // SAFETY: Each element in the array is an AXUIElementRef.
            // We retain it because CFArray elements are not owned by us.
            let element = unsafe {
                let ptr = *array.get(i).unwrap();
                AXWindow::from_raw_retained(ptr as AXUIElementRef)
            };
            windows.push(element);
        }

        Ok(windows)
    }

    /// Get the raw element ref for observer registration.
    pub fn as_raw(&self) -> AXUIElementRef {
        self.element
    }
}

impl Drop for AXApp {
    fn drop(&mut self) {
        if !self.element.is_null() {
            // SAFETY: We own a retained reference from AXUIElementCreateApplication.
            unsafe { CFRelease(self.element as CFTypeRef) };
        }
    }
}

// -- Private API resolution --

type AXUIElementGetWindowFn =
    unsafe extern "C" fn(element: AXUIElementRef, wid: *mut u32) -> AXError;

/// Resolve the private `_AXUIElementGetWindow` symbol at runtime via dlsym.
///
/// Returns the function pointer or an error if the symbol is not available
/// (e.g., on a future macOS version that removes it).
fn resolve_ax_get_window() -> Result<AXUIElementGetWindowFn> {
    use std::sync::OnceLock;

    static FUNC: OnceLock<Option<AXUIElementGetWindowFn>> = OnceLock::new();

    let func = FUNC.get_or_init(|| {
        // SAFETY: dlsym with RTLD_DEFAULT searches all loaded libraries.
        // _AXUIElementGetWindow has existed since macOS 10.10 and is used
        // by Aerospace, yabai, and Paneru without incident.
        unsafe {
            let symbol = libc::dlsym(
                libc::RTLD_DEFAULT,
                c"_AXUIElementGetWindow".as_ptr(),
            );
            if symbol.is_null() {
                None
            } else {
                Some(std::mem::transmute::<*mut std::ffi::c_void, AXUIElementGetWindowFn>(symbol))
            }
        }
    });

    func.ok_or_else(|| {
        anyhow!(
            "_AXUIElementGetWindow not available on this macOS version. \
             Window ID resolution is not possible."
        )
    })
}
