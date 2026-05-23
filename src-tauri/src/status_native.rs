//! Native status overlay window FFI bindings.
//! Platform-specific implementations:
//! - Windows: Win32 API + GDI+
//! - macOS: Cocoa/AppKit
//! - Linux: GTK3 + Cairo

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
use std::ffi::{c_char, CString};
#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
use std::sync::{mpsc, Mutex, OnceLock};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusType {
    Recording = 0,
    Transcribing = 1,
    Completed = 2,
    Error = 3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusActionSet {
    None = 0,
    Retry = 1,
    RetryCancel = 2,
}

#[repr(C)]
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatusOverlayAction {
    Retry = 0,
    Cancel = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayAction {
    Retry,
    Cancel,
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
extern "C" {
    fn status_overlay_init() -> i32;
    fn status_overlay_show(status: StatusType, text: *const c_char);
    fn status_overlay_show_actions(
        status: StatusType,
        text: *const c_char,
        actions: StatusActionSet,
    );
    fn status_overlay_set_action_callback(callback: Option<extern "C" fn(StatusOverlayAction)>);
    fn status_overlay_hide();
    fn status_overlay_cleanup();
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
static ACTION_SENDER: OnceLock<Mutex<Option<mpsc::Sender<OverlayAction>>>> = OnceLock::new();

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
extern "C" fn handle_overlay_action(action: StatusOverlayAction) {
    let action = match action {
        StatusOverlayAction::Retry => OverlayAction::Retry,
        StatusOverlayAction::Cancel => OverlayAction::Cancel,
    };
    if let Some(sender_lock) = ACTION_SENDER.get() {
        if let Ok(sender_guard) = sender_lock.lock() {
            if let Some(sender) = sender_guard.as_ref() {
                let _ = sender.send(action);
            }
        }
    }
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

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
pub fn show_with_actions(status: StatusType, text: &str, actions: StatusActionSet) {
    if let Ok(c_text) = CString::new(text) {
        unsafe { status_overlay_show_actions(status, c_text.as_ptr(), actions) }
    }
}

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
pub fn set_action_sender(sender: mpsc::Sender<OverlayAction>) {
    let sender_lock = ACTION_SENDER.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = sender_lock.lock() {
        *guard = Some(sender);
    }
    unsafe { status_overlay_set_action_callback(Some(handle_overlay_action)) }
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
pub fn show_with_actions(_status: StatusType, _text: &str, _actions: StatusActionSet) {}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
pub fn set_action_sender(_sender: std::sync::mpsc::Sender<OverlayAction>) {}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
pub fn hide() {}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
pub fn cleanup() {}
