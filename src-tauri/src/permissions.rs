use serde::Serialize;

#[cfg(target_os = "macos")]
use crate::recorder;

#[cfg(target_os = "macos")]
extern "C" {
    fn macos_accessibility_permission_status_code() -> i32;
    fn macos_request_accessibility_permission_code() -> i32;
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MacosPermissionItem {
    pub id: &'static str,
    pub status: &'static str,
    pub required: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MacosPermissionStatus {
    pub supported: bool,
    pub microphone: MacosPermissionItem,
    pub accessibility: MacosPermissionItem,
}

pub fn macos_permission_status() -> MacosPermissionStatus {
    #[cfg(target_os = "macos")]
    {
        return MacosPermissionStatus {
            supported: true,
            microphone: microphone_permission_item(false),
            accessibility: accessibility_permission_item(false),
        };
    }

    #[cfg(not(target_os = "macos"))]
    {
        MacosPermissionStatus {
            supported: false,
            microphone: unsupported_item("microphone"),
            accessibility: unsupported_item("accessibility"),
        }
    }
}

pub fn request_macos_permission(permission_id: &str) -> MacosPermissionStatus {
    #[cfg(target_os = "macos")]
    {
        match permission_id {
            "microphone" => {
                let _ = recorder::request_microphone_permission();
            }
            "accessibility" => {
                let _ = accessibility_permission_item(true);
            }
            _ => {}
        }
        return macos_permission_status();
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = permission_id;
        macos_permission_status()
    }
}

pub fn missing_recording_permission_message() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let item = microphone_permission_item(false);
        if is_approved(item.status) {
            return None;
        }
        return Some(match item.status {
            "notDetermined" => {
                "麦克风权限尚未批准，请先在 macOS 权限页面允许麦克风访问后再开始录音".to_string()
            }
            "restricted" => "macOS 当前限制了麦克风访问，无法开始录音".to_string(),
            "denied" => {
                "麦克风权限未批准，请在“系统设置 > 隐私与安全性 > 麦克风”中允许 VTT Keyboard"
                    .to_string()
            }
            _ => "无法确认麦克风权限状态，请先检查 macOS 权限页面".to_string(),
        });
    }

    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

pub fn missing_accessibility_permission_message() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let item = accessibility_permission_item(false);
        if is_approved(item.status) {
            return None;
        }
        return Some(
            "辅助功能权限未批准，已复制文本但不会自动粘贴。请先在 macOS 权限页面允许辅助功能访问"
                .to_string(),
        );
    }

    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

pub fn is_approved(status: &str) -> bool {
    status == "authorized" || status == "approved"
}

#[cfg(target_os = "macos")]
fn microphone_permission_item(request: bool) -> MacosPermissionItem {
    let status = if request {
        recorder::request_microphone_permission().status
    } else {
        recorder::microphone_permission_status().status
    };
    MacosPermissionItem {
        id: "microphone",
        status: microphone_status_name(&status),
        required: true,
    }
}

#[cfg(target_os = "macos")]
fn accessibility_permission_item(request: bool) -> MacosPermissionItem {
    let code = if request {
        unsafe { macos_request_accessibility_permission_code() }
    } else {
        unsafe { macos_accessibility_permission_status_code() }
    };
    MacosPermissionItem {
        id: "accessibility",
        status: accessibility_status_name(code),
        required: true,
    }
}

#[cfg(target_os = "macos")]
fn microphone_status_name(status: &str) -> &'static str {
    match status {
        "notDetermined" => "notDetermined",
        "restricted" => "restricted",
        "denied" => "denied",
        "authorized" => "authorized",
        _ => "unknown",
    }
}

#[cfg(target_os = "macos")]
fn accessibility_status_name(status: i32) -> &'static str {
    match status {
        0 => "denied",
        1 => "approved",
        _ => "unknown",
    }
}

#[cfg(not(target_os = "macos"))]
fn unsupported_item(id: &'static str) -> MacosPermissionItem {
    MacosPermissionItem {
        id,
        status: "unsupported",
        required: false,
    }
}

#[cfg(test)]
mod tests {
    use super::is_approved;

    #[test]
    fn approved_status_accepts_microphone_and_accessibility_values() {
        assert!(is_approved("authorized"));
        assert!(is_approved("approved"));
        assert!(!is_approved("denied"));
        assert!(!is_approved("notDetermined"));
        assert!(!is_approved("unknown"));
    }
}
