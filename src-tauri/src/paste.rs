use enigo::{Enigo, KeyboardControllable, Key};
use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PasteError {
    #[error("剪贴板写入失败: {0}")]
    Clipboard(String),
    #[error("模拟粘贴失败: {0}")]
    Paste(String),
}

pub fn write_text(app: &AppHandle, text: &str) -> Result<(), PasteError> {
    app.clipboard()
        .write_text(text.to_string())
        .map_err(|err| PasteError::Clipboard(err.to_string()))?;
    Ok(())
}

pub fn write_and_paste(app: &AppHandle, text: &str) -> Result<(), PasteError> {
    write_text(app, text)?;
    send_paste_shortcut().map_err(|err| PasteError::Paste(err.to_string()))?;
    Ok(())
}

fn send_paste_shortcut() -> Result<(), String> {
    let mut enigo = Enigo::new();
    #[cfg(target_os = "macos")]
    {
        enigo.key_down(Key::Meta);
        enigo.key_click(Key::Layout('v'));
        enigo.key_up(Key::Meta);
        return Ok(());
    }
    #[cfg(not(target_os = "macos"))]
    {
        enigo.key_down(Key::Control);
        enigo.key_click(Key::Layout('v'));
        enigo.key_up(Key::Control);
        return Ok(());
    }
}
