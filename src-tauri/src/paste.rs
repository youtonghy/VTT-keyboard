use crate::permissions;
use arboard::Clipboard;
#[cfg(not(target_os = "macos"))]
use enigo::{Enigo, Key, KeyboardControllable};
use std::{thread, time::Duration};
use thiserror::Error;

const PASTEBOARD_SETTLE_DELAY: Duration = Duration::from_millis(80);

#[cfg(target_os = "macos")]
const MACOS_ACCESSIBILITY_MISSING: i32 = 1;

#[cfg(target_os = "macos")]
extern "C" {
    fn macos_send_paste_shortcut(prompt_for_accessibility: i32) -> i32;
}

#[derive(Debug, Error)]
pub enum PasteError {
    #[error("剪贴板写入失败: {0}")]
    Clipboard(String),
    #[error("模拟粘贴失败: {0}")]
    Paste(String),
}

pub fn write_text(text: &str) -> Result<(), PasteError> {
    let mut clipboard = Clipboard::new().map_err(|err| PasteError::Clipboard(err.to_string()))?;
    clipboard
        .set_text(text.to_string())
        .map_err(|err| PasteError::Clipboard(err.to_string()))?;
    Ok(())
}

pub fn write_and_paste(text: &str) -> Result<(), PasteError> {
    write_text(text)?;
    if let Some(message) = permissions::missing_accessibility_permission_message() {
        return Err(PasteError::Paste(message));
    }
    thread::sleep(PASTEBOARD_SETTLE_DELAY);
    send_paste_shortcut().map_err(|err| PasteError::Paste(err.to_string()))?;
    Ok(())
}

fn send_paste_shortcut() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let status = unsafe { macos_send_paste_shortcut(0) };
        if status == 0 {
            return Ok(());
        }
        return Err(describe_macos_paste_shortcut_status(status));
    }
    #[cfg(not(target_os = "macos"))]
    {
        let mut enigo = Enigo::new();
        enigo.key_down(Key::Control);
        enigo.key_click(Key::Layout('v'));
        enigo.key_up(Key::Control);
        Ok(())
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
}
