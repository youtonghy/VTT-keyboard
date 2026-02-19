//! Native status overlay window FFI bindings.
//! Platform-specific implementations:
//! - Windows: Win32 API + GDI+
//! - macOS: Cocoa/AppKit
//! - Linux: GTK3 + Cairo

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
use std::ffi::CString;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusType {
    Recording = 0,
    Transcribing = 1,
    Completed = 2,
    Error = 3,
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
extern "C" {
    fn status_overlay_init() -> i32;
    fn status_overlay_show(status: StatusType, text: *const i8);
    fn status_overlay_hide();
    fn status_overlay_cleanup();
}

/// Initialize the native status overlay.
/// Returns true on success, false on failure.
/// Should be called once at application startup.
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
pub fn init() -> bool {
    unsafe { status_overlay_init() == 0 }
}

/// Show the status overlay with the given status and text.
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
pub fn show(status: StatusType, text: &str) {
    if let Ok(c_text) = CString::new(text) {
        unsafe { status_overlay_show(status, c_text.as_ptr()) }
    }
}

/// Hide the status overlay.
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
pub fn hide() {
    unsafe { status_overlay_hide() }
}

/// Cleanup the native status overlay.
/// Should be called once at application exit.
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
pub fn cleanup() {
    unsafe { status_overlay_cleanup() }
}

// Stub implementations for unsupported platforms
#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
pub fn init() -> bool {
    #[cfg(debug_assertions)]
    eprintln!("Native status overlay is not supported on this platform");
    false
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
pub fn show(_status: StatusType, _text: &str) {}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
pub fn hide() {}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
pub fn cleanup() {}
