//! Linux-specific input helpers: session detection, XDG portal host-app
//! registration and the Wayland paste backend chain
//! (RemoteDesktop portal -> wtype -> ydotool -> copy only).

use crate::paste::PasteOutcome;
use crate::settings::PasteShortcut;
use ashpd::desktop::remote_desktop::{DeviceType, KeyState, RemoteDesktop, SelectDevicesOptions};
use ashpd::desktop::{PersistMode, Session};
use std::future::Future;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tauri::async_runtime;

pub const APP_ID: &str = "com.youtonghy.vtt-keyboard";

/// The first portal request may show a consent dialog, so allow time for the user.
const PORTAL_TIMEOUT: Duration = Duration::from_secs(90);
const TOOL_TIMEOUT: Duration = Duration::from_secs(3);

const XK_SHIFT_L: i32 = 0xffe1;
const XK_CONTROL_L: i32 = 0xffe3;
const XK_INSERT: i32 = 0xff63;
const XK_V: i32 = 0x0076;

static TOKEN_PATH: OnceLock<PathBuf> = OnceLock::new();
static HOST_APP_REGISTERED: async_runtime::Mutex<bool> = async_runtime::Mutex::const_new(false);
static PORTAL_SESSION: async_runtime::Mutex<Option<PortalSession>> =
    async_runtime::Mutex::const_new(None);
/// Set once the user denies the portal or it is missing, to avoid prompting on every paste.
static PORTAL_UNAVAILABLE: AtomicBool = AtomicBool::new(false);

struct PortalSession {
    proxy: RemoteDesktop,
    session: Session<RemoteDesktop>,
}

pub fn init(app_data_dir: PathBuf) {
    let _ = TOKEN_PATH.set(app_data_dir.join("remote-desktop-restore-token"));
}

pub fn is_wayland_session() -> bool {
    static WAYLAND: OnceLock<bool> = OnceLock::new();
    *WAYLAND.get_or_init(|| {
        session_is_wayland(
            std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
            std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
        )
    })
}

fn session_is_wayland(wayland_display: Option<&str>, session_type: Option<&str>) -> bool {
    wayland_display.is_some_and(|value| !value.is_empty())
        || session_type.is_some_and(|value| value.eq_ignore_ascii_case("wayland"))
}

/// Non-sandboxed apps must announce their app id before using portals, otherwise
/// permissions cannot be stored per app. Errors are ignored: older portals lack
/// the registry and still work without it.
pub async fn register_host_app_once() {
    let mut registered = HOST_APP_REGISTERED.lock().await;
    if *registered {
        return;
    }
    *registered = true;
    if let Ok(app_id) = ashpd::AppID::try_from(APP_ID) {
        let _ = ashpd::register_host_app(app_id).await;
    }
}

/// Runs a portal future on Tauri's tokio runtime from synchronous code. Safe to
/// call from any thread, including runtime worker threads.
pub fn block_on_runtime<F, T>(future: F, timeout: Duration) -> Option<T>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    async_runtime::spawn(async move {
        let _ = tx.send(future.await);
    });
    rx.recv_timeout(timeout).ok()
}

pub fn send_paste_wayland(shortcut: PasteShortcut) -> PasteOutcome {
    if !PORTAL_UNAVAILABLE.load(Ordering::Relaxed) {
        let keysyms = portal_keysyms(shortcut);
        match block_on_runtime(portal_send_keys(keysyms), PORTAL_TIMEOUT) {
            Some(Ok(())) => return PasteOutcome::Pasted,
            Some(Err(_err)) => {
                #[cfg(debug_assertions)]
                eprintln!("[DEV] RemoteDesktop portal paste failed: {_err}");
                PORTAL_UNAVAILABLE.store(true, Ordering::Relaxed);
            }
            None => {
                #[cfg(debug_assertions)]
                eprintln!("[DEV] RemoteDesktop portal paste timed out");
            }
        }
    }
    if run_tool("wtype", &wtype_args(shortcut)) {
        return PasteOutcome::Pasted;
    }
    if run_tool("ydotool", &ydotool_args(shortcut)) {
        return PasteOutcome::Pasted;
    }
    PasteOutcome::CopiedOnly
}

/// Returns (modifiers, key) as X keysyms.
fn portal_keysyms(shortcut: PasteShortcut) -> (&'static [i32], i32) {
    match shortcut {
        PasteShortcut::CtrlV => (&[XK_CONTROL_L], XK_V),
        PasteShortcut::CtrlShiftV => (&[XK_CONTROL_L, XK_SHIFT_L], XK_V),
        PasteShortcut::ShiftInsert => (&[XK_SHIFT_L], XK_INSERT),
    }
}

async fn portal_send_keys(keysyms: (&'static [i32], i32)) -> Result<(), String> {
    let mut guard = PORTAL_SESSION.lock().await;
    if guard.is_none() {
        *guard = Some(start_portal_session().await?);
    }
    let first_try = send_keysyms(guard.as_ref().expect("session set above"), keysyms).await;
    if first_try.is_ok() {
        return Ok(());
    }
    // The session may have been closed by the compositor; start a new one once.
    *guard = None;
    let session = start_portal_session().await?;
    send_keysyms(&session, keysyms).await?;
    *guard = Some(session);
    Ok(())
}

async fn start_portal_session() -> Result<PortalSession, String> {
    register_host_app_once().await;
    let proxy = RemoteDesktop::new().await.map_err(|err| err.to_string())?;
    let session = proxy
        .create_session(Default::default())
        .await
        .map_err(|err| err.to_string())?;
    let restore_token = read_restore_token();
    proxy
        .select_devices(
            &session,
            SelectDevicesOptions::default()
                .set_devices(ashpd::enumflags2::BitFlags::from(DeviceType::Keyboard))
                .set_persist_mode(PersistMode::ExplicitlyRevoked)
                .set_restore_token(restore_token.as_deref()),
        )
        .await
        .map_err(|err| err.to_string())?
        .response()
        .map_err(|err| err.to_string())?;
    let selected = proxy
        .start(&session, None, Default::default())
        .await
        .map_err(|err| err.to_string())?
        .response()
        .map_err(|err| err.to_string())?;
    // Restore tokens are single-use; persist the fresh one for the next launch.
    if let Some(token) = selected.restore_token() {
        write_restore_token(token);
    }
    Ok(PortalSession { proxy, session })
}

async fn send_keysyms(
    portal: &PortalSession,
    (modifiers, key): (&'static [i32], i32),
) -> Result<(), String> {
    let send = |keysym: i32, state: KeyState| {
        portal
            .proxy
            .notify_keyboard_keysym(&portal.session, keysym, state, Default::default())
    };
    for modifier in modifiers {
        send(*modifier, KeyState::Pressed)
            .await
            .map_err(|err| err.to_string())?;
    }
    let result = async {
        send(key, KeyState::Pressed).await?;
        send(key, KeyState::Released).await
    }
    .await;
    for modifier in modifiers.iter().rev() {
        let _ = send(*modifier, KeyState::Released).await;
    }
    result.map_err(|err| err.to_string())
}

fn read_restore_token() -> Option<String> {
    let path = TOKEN_PATH.get()?;
    let token = std::fs::read_to_string(path).ok()?;
    let token = token.trim();
    (!token.is_empty()).then(|| token.to_string())
}

fn write_restore_token(token: &str) {
    if let Some(path) = TOKEN_PATH.get() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, token);
    }
}

fn wtype_args(shortcut: PasteShortcut) -> Vec<&'static str> {
    match shortcut {
        PasteShortcut::CtrlV => vec!["-M", "ctrl", "-k", "v", "-m", "ctrl"],
        PasteShortcut::CtrlShiftV => vec![
            "-M", "ctrl", "-M", "shift", "-k", "v", "-m", "shift", "-m", "ctrl",
        ],
        PasteShortcut::ShiftInsert => vec!["-M", "shift", "-k", "Insert", "-m", "shift"],
    }
}

/// ydotool takes Linux input event codes: 29 LEFTCTRL, 42 LEFTSHIFT, 47 V, 110 INSERT.
fn ydotool_args(shortcut: PasteShortcut) -> Vec<&'static str> {
    match shortcut {
        PasteShortcut::CtrlV => vec!["key", "29:1", "47:1", "47:0", "29:0"],
        PasteShortcut::CtrlShiftV => vec!["key", "29:1", "42:1", "47:1", "47:0", "42:0", "29:0"],
        PasteShortcut::ShiftInsert => vec!["key", "42:1", "110:1", "110:0", "42:0"],
    }
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

fn run_tool(name: &str, args: &[&str]) -> bool {
    let Some(path) = find_in_path(name) else {
        return false;
    };
    let Ok(mut child) = Command::new(path)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    let deadline = Instant::now() + TOOL_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_wayland_from_display_or_session_type() {
        assert!(session_is_wayland(Some("wayland-0"), None));
        assert!(session_is_wayland(None, Some("wayland")));
        assert!(session_is_wayland(Some(""), Some("Wayland")));
        assert!(!session_is_wayland(Some(""), Some("x11")));
        assert!(!session_is_wayland(None, None));
    }

    #[test]
    fn portal_keysyms_match_paste_shortcut() {
        assert_eq!(
            portal_keysyms(PasteShortcut::CtrlV),
            (&[XK_CONTROL_L][..], XK_V)
        );
        assert_eq!(
            portal_keysyms(PasteShortcut::CtrlShiftV),
            (&[XK_CONTROL_L, XK_SHIFT_L][..], XK_V)
        );
        assert_eq!(
            portal_keysyms(PasteShortcut::ShiftInsert),
            (&[XK_SHIFT_L][..], XK_INSERT)
        );
    }

    #[test]
    fn tool_args_press_and_release_modifiers() {
        assert_eq!(
            wtype_args(PasteShortcut::ShiftInsert),
            ["-M", "shift", "-k", "Insert", "-m", "shift"]
        );
        let ydotool = ydotool_args(PasteShortcut::CtrlShiftV);
        assert_eq!(ydotool.first(), Some(&"key"));
        assert_eq!(ydotool.iter().filter(|arg| arg.ends_with(":1")).count(), 3);
        assert_eq!(ydotool.iter().filter(|arg| arg.ends_with(":0")).count(), 3);
    }

    #[test]
    fn app_id_is_valid_for_portal_registry() {
        assert!(ashpd::AppID::try_from(APP_ID).is_ok());
    }

    #[test]
    fn missing_tool_is_reported_as_failure() {
        assert!(!run_tool("vtt-keyboard-definitely-missing-tool", &[]));
    }
}
