//! Native status overlay window FFI bindings.
//! Windows-only implementation using Win32 API.

#[cfg(target_os = "windows")]
use std::ffi::CString;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusType {
    Recording = 0,
    Transcribing = 1,
    Completed = 2,
    Error = 3,
}

#[cfg(target_os = "windows")]
extern "C" {
    fn status_overlay_init() -> i32;
    fn status_overlay_show(status: StatusType, text: *const i8);
    fn status_overlay_hide();
    fn status_overlay_cleanup();
}

/// Initialize the native status overlay.
/// Returns true on success, false on failure.
/// Should be called once at application startup.
#[cfg(target_os = "windows")]
pub fn init() -> bool {
    unsafe { status_overlay_init() == 0 }
}

/// Show the status overlay with the given status and text.
#[cfg(target_os = "windows")]
pub fn show(status: StatusType, text: &str) {
    if let Ok(c_text) = CString::new(text) {
        unsafe { status_overlay_show(status, c_text.as_ptr()) }
    }
}

/// Hide the status overlay.
#[cfg(target_os = "windows")]
pub fn hide() {
    unsafe { status_overlay_hide() }
}

/// Cleanup the native status overlay.
/// Should be called once at application exit.
#[cfg(target_os = "windows")]
pub fn cleanup() {
    unsafe { status_overlay_cleanup() }
}

// Non-Windows stub implementations
#[cfg(not(target_os = "windows"))]
pub fn init() -> bool {
    eprintln!("Native status overlay is not supported on this platform");
    false
}

#[cfg(not(target_os = "windows"))]
pub fn show(_status: StatusType, _text: &str) {}

#[cfg(not(target_os = "windows"))]
pub fn hide() {}

#[cfg(not(target_os = "windows"))]
pub fn cleanup() {}
