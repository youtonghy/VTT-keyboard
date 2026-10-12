//! Global recording shortcut on Wayland via the XDG GlobalShortcuts portal.
//! Activated/Deactivated signals are forwarded to the frontend as the same
//! Pressed/Released states the X11 global-shortcut plugin produces.

use crate::linux_input;
use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
use ashpd::desktop::Session;
use futures_util::StreamExt;
use serde::Serialize;
use tauri::async_runtime::{self, JoinHandle};
use tauri::{AppHandle, Emitter};

pub const RECORDING_SHORTCUT_EVENT: &str = "recording-shortcut";
const SHORTCUT_ID: &str = "toggle-recording";

struct Registration {
    session: Session<GlobalShortcuts>,
    listener: JoinHandle<()>,
}

static REGISTRATION: async_runtime::Mutex<Option<Registration>> =
    async_runtime::Mutex::const_new(None);

#[derive(Clone, Serialize)]
pub struct ShortcutStateEvent {
    pub state: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WaylandShortcutInfo {
    /// Trigger actually assigned by the desktop, which may differ from the requested one.
    pub trigger: Option<String>,
}

pub fn emit_state(app: &AppHandle, state: &'static str) {
    let _ = app.emit(RECORDING_SHORTCUT_EVENT, ShortcutStateEvent { state });
}

pub async fn register(app: AppHandle, accelerator: String) -> Result<WaylandShortcutInfo, String> {
    unregister().await;
    linux_input::register_host_app_once().await;

    let proxy = GlobalShortcuts::new()
        .await
        .map_err(|err| format!("GlobalShortcuts portal unavailable: {err}"))?;
    let session = proxy
        .create_session(Default::default())
        .await
        .map_err(|err| err.to_string())?;
    let preferred = to_portal_trigger(&accelerator);
    let shortcut = NewShortcut::new(SHORTCUT_ID, "Start/stop voice input")
        .preferred_trigger(preferred.as_deref());
    let bound = proxy
        .bind_shortcuts(&session, &[shortcut], None, Default::default())
        .await
        .map_err(|err| err.to_string())?
        .response()
        .map_err(|err| err.to_string())?;
    let trigger = bound
        .shortcuts()
        .iter()
        .find(|item| item.id() == SHORTCUT_ID)
        .map(|item| item.trigger_description().to_string())
        .filter(|value| !value.is_empty());

    let mut activated = proxy
        .receive_activated()
        .await
        .map_err(|err| err.to_string())?;
    let mut deactivated = proxy
        .receive_deactivated()
        .await
        .map_err(|err| err.to_string())?;
    let listener = async_runtime::spawn(async move {
        // Keep the proxy alive for as long as signals are being received.
        let _proxy = proxy;
        loop {
            tokio::select! {
                Some(event) = activated.next() => {
                    if event.shortcut_id() == SHORTCUT_ID {
                        emit_state(&app, "Pressed");
                    }
                }
                Some(event) = deactivated.next() => {
                    if event.shortcut_id() == SHORTCUT_ID {
                        emit_state(&app, "Released");
                    }
                }
                else => break,
            }
        }
    });

    *REGISTRATION.lock().await = Some(Registration { session, listener });
    Ok(WaylandShortcutInfo { trigger })
}

pub async fn unregister() {
    if let Some(registration) = REGISTRATION.lock().await.take() {
        registration.listener.abort();
        let _ = registration.session.close().await;
    }
}

/// Converts a Tauri accelerator ("CommandOrControl+Shift+Space") into the XDG
/// shortcuts format ("CTRL+SHIFT+space").
fn to_portal_trigger(accelerator: &str) -> Option<String> {
    let mut parts = Vec::new();
    for part in accelerator
        .split('+')
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        let mapped = match part.to_ascii_lowercase().as_str() {
            "commandorcontrol" | "cmdorctrl" | "control" | "ctrl" | "command" | "cmd" => {
                "CTRL".to_string()
            }
            "alt" | "option" => "ALT".to_string(),
            "shift" => "SHIFT".to_string(),
            "super" | "meta" | "logo" => "LOGO".to_string(),
            _ => to_portal_key(part),
        };
        parts.push(mapped);
    }
    (!parts.is_empty()).then(|| parts.join("+"))
}

fn to_portal_key(key: &str) -> String {
    match key {
        "Space" => "space".to_string(),
        "Enter" => "Return".to_string(),
        "Up" | "Down" | "Left" | "Right" => key.to_string(),
        _ if key.chars().count() == 1 => key.to_ascii_lowercase(),
        _ => key.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::to_portal_trigger;

    #[test]
    fn converts_tauri_accelerator_to_xdg_trigger() {
        assert_eq!(
            to_portal_trigger("CommandOrControl+Shift+Space").as_deref(),
            Some("CTRL+SHIFT+space")
        );
        assert_eq!(to_portal_trigger("Alt+F9").as_deref(), Some("ALT+F9"));
        assert_eq!(to_portal_trigger("Super+V").as_deref(), Some("LOGO+v"));
        assert_eq!(
            to_portal_trigger("Control+Enter").as_deref(),
            Some("CTRL+Return")
        );
        assert_eq!(to_portal_trigger(""), None);
    }
}
