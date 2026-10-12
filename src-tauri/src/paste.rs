use crate::permissions;
use crate::settings::PasteShortcut;
use arboard::Clipboard;
#[cfg(not(target_os = "macos"))]
use enigo::{Direction, Enigo, Key, Keyboard, Settings as EnigoSettings};
use std::sync::{Mutex, OnceLock};
use std::{thread, time::Duration};
use thiserror::Error;

const PASTEBOARD_SETTLE_DELAY: Duration = Duration::from_millis(80);

#[cfg(target_os = "macos")]
const MACOS_ACCESSIBILITY_MISSING: i32 = 1;

#[cfg(target_os = "macos")]
extern "C" {
    fn macos_send_paste_shortcut(prompt_for_accessibility: i32) -> i32;
}

/// Process-wide clipboard handle. On X11 the clipboard contents are served by
/// this process, so dropping the handle right after writing can lose the text
/// before the target app reads it (no clipboard manager on i3/sway etc.).
static CLIPBOARD: OnceLock<Mutex<Option<Clipboard>>> = OnceLock::new();

#[derive(Debug, Error)]
pub enum PasteError {
    #[error("剪贴板写入失败: {0}")]
    Clipboard(String),
    #[error("模拟粘贴失败: {0}")]
    Paste(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteOutcome {
    Pasted,
    /// Text is on the clipboard but no input backend could send the paste keys
    /// (e.g. Wayland without portal/wtype/ydotool). The user pastes manually.
    CopiedOnly,
}

pub fn write_text(text: &str) -> Result<(), PasteError> {
    write_text_with_primary(text, false)
}

fn write_text_with_primary(text: &str, also_primary: bool) -> Result<(), PasteError> {
    let lock = CLIPBOARD.get_or_init(|| Mutex::new(None));
    let mut guard = lock
        .lock()
        .map_err(|err| PasteError::Clipboard(err.to_string()))?;

    let mut last_error = String::new();
    // Retry once with a fresh handle in case the cached connection went stale.
    for _ in 0..2 {
        if guard.is_none() {
            match Clipboard::new() {
                Ok(clipboard) => *guard = Some(clipboard),
                Err(err) => return Err(PasteError::Clipboard(err.to_string())),
            }
        }
        let clipboard = guard.as_mut().expect("clipboard initialized above");
        match set_clipboard_text(clipboard, text, also_primary) {
            Ok(()) => return Ok(()),
            Err(err) => {
                last_error = err;
                *guard = None;
            }
        }
    }
    Err(PasteError::Clipboard(last_error))
}

fn set_clipboard_text(
    clipboard: &mut Clipboard,
    text: &str,
    also_primary: bool,
) -> Result<(), String> {
    clipboard
        .set_text(text.to_string())
        .map_err(|err| err.to_string())?;
    #[cfg(target_os = "linux")]
    if also_primary {
        use arboard::{LinuxClipboardKind, SetExtLinux};
        clipboard
            .set()
            .clipboard(LinuxClipboardKind::Primary)
            .text(text.to_string())
            .map_err(|err| err.to_string())?;
    }
    #[cfg(not(target_os = "linux"))]
    let _ = also_primary;
    Ok(())
}

pub fn write_and_paste(text: &str, shortcut: PasteShortcut) -> Result<PasteOutcome, PasteError> {
    let shortcut = effective_shortcut(shortcut);
    write_text_with_primary(text, shortcut == PasteShortcut::ShiftInsert)?;
    if let Some(message) = permissions::missing_accessibility_permission_message() {
        return Err(PasteError::Paste(message));
    }
    thread::sleep(PASTEBOARD_SETTLE_DELAY);
    send_paste_shortcut(shortcut).map_err(PasteError::Paste)
}

/// The paste key setting only applies on Linux; other platforms keep Ctrl/Cmd+V.
fn effective_shortcut(shortcut: PasteShortcut) -> PasteShortcut {
    if cfg!(target_os = "linux") {
        shortcut
    } else {
        PasteShortcut::CtrlV
    }
}

fn send_paste_shortcut(shortcut: PasteShortcut) -> Result<PasteOutcome, String> {
    #[cfg(target_os = "macos")]
    {
        let _ = shortcut;
        let status = unsafe { macos_send_paste_shortcut(0) };
        if status == 0 {
            return Ok(PasteOutcome::Pasted);
        }
        return Err(describe_macos_paste_shortcut_status(status));
    }
    #[cfg(not(target_os = "macos"))]
    {
        #[cfg(target_os = "linux")]
        if crate::linux_input::is_wayland_session() {
            return Ok(crate::linux_input::send_paste_wayland(shortcut));
        }
        send_paste_with_enigo(shortcut).map(|()| PasteOutcome::Pasted)
    }
}

#[cfg(not(target_os = "macos"))]
fn send_paste_with_enigo(shortcut: PasteShortcut) -> Result<(), String> {
    let mut enigo = Enigo::new(&EnigoSettings::default()).map_err(|err| err.to_string())?;
    let (modifiers, key) = enigo_combo(shortcut);
    for modifier in modifiers {
        enigo
            .key(*modifier, Direction::Press)
            .map_err(|err| err.to_string())?;
    }
    let click = enigo.key(key, Direction::Click);
    // Always release modifiers, even if the main key failed.
    for modifier in modifiers.iter().rev() {
        let _ = enigo.key(*modifier, Direction::Release);
    }
    click.map_err(|err| err.to_string())
}

#[cfg(not(target_os = "macos"))]
fn enigo_combo(shortcut: PasteShortcut) -> (&'static [Key], Key) {
    // Windows needs the virtual key; Unicode input does not combine with Ctrl.
    #[cfg(target_os = "windows")]
    let v_key = Key::V;
    #[cfg(not(target_os = "windows"))]
    let v_key = Key::Unicode('v');

    match shortcut {
        PasteShortcut::CtrlV => (&[Key::Control], v_key),
        PasteShortcut::CtrlShiftV => (&[Key::Control, Key::Shift], v_key),
        PasteShortcut::ShiftInsert => (&[Key::Shift], Key::Insert),
    }
}

#[cfg(target_os = "macos")]
fn describe_macos_paste_shortcut_status(status: i32) -> String {
    match status {
        MACOS_ACCESSIBILITY_MISSING => {
            "macOS 辅助功能权限未开启，已请求系统授权。请在“系统设置 > 隐私与安全性 > 辅助功能”中允许 VTT Keyboard 后重试".to_string()
        }
        -1 => "无法创建 macOS 键盘事件".to_string(),
        -2 => "macOS 粘贴快捷键发送失败".to_string(),
        other => format!("macOS 粘贴快捷键发送失败: {other}"),
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "macos")]
    use super::{describe_macos_paste_shortcut_status, MACOS_ACCESSIBILITY_MISSING};

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_paste_shortcut_status_explains_accessibility_permission() {
        let message = describe_macos_paste_shortcut_status(MACOS_ACCESSIBILITY_MISSING);

        assert!(message.contains("辅助功能权限未开启"));
        assert!(message.contains("系统设置"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_paste_shortcut_status_includes_unknown_code() {
        let message = describe_macos_paste_shortcut_status(42);

        assert!(message.contains("42"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_respects_configured_paste_shortcut() {
        use super::effective_shortcut;
        use crate::settings::PasteShortcut;

        assert_eq!(
            effective_shortcut(PasteShortcut::ShiftInsert),
            PasteShortcut::ShiftInsert
        );
        assert_eq!(
            effective_shortcut(PasteShortcut::CtrlShiftV),
            PasteShortcut::CtrlShiftV
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn shift_insert_uses_shift_modifier_only() {
        use super::enigo_combo;
        use crate::settings::PasteShortcut;
        use enigo::Key;

        let (modifiers, key) = enigo_combo(PasteShortcut::ShiftInsert);
        assert_eq!(modifiers, &[Key::Shift]);
        assert_eq!(key, Key::Insert);
    }
}
