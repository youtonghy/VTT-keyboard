mod audio_processing;
mod openai;
mod paste;
mod processing;
mod recorder;
mod settings;
mod triggers;

use recorder::RecorderService;
use settings::{Settings, SettingsStore};
use std::fs;
use tauri::{
    Manager, PhysicalPosition, PhysicalSize, Position, Size, State, WebviewUrl,
    WebviewWindowBuilder, WindowEvent,
};
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;

#[derive(Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrayLabels {
    show_settings: String,
    quit: String,
}

struct AppState {
    recorder: RecorderService,
    settings_store: SettingsStore,
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> Result<Settings, String> {
    state
        .settings_store
        .load()
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn update_settings(state: State<AppState>, settings: Settings) -> Result<(), String> {
    state
        .settings_store
        .save(&settings)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn export_settings(state: State<AppState>, path: String) -> Result<(), String> {
    let settings = state
        .settings_store
        .load()
        .map_err(|err| err.to_string())?;
    let data = serde_json::to_string_pretty(&settings).map_err(|err| err.to_string())?;
    fs::write(path, data).map_err(|err| err.to_string())?;
    Ok(())
}

#[tauri::command]
fn import_settings(state: State<AppState>, path: String) -> Result<Settings, String> {
    let data = fs::read_to_string(path).map_err(|err| err.to_string())?;
    let settings: Settings = serde_json::from_str(&data).map_err(|err| err.to_string())?;
    state
        .settings_store
        .save(&settings)
        .map_err(|err| err.to_string())?;
    Ok(settings)
}

#[tauri::command]
fn start_recording(app: tauri::AppHandle, state: State<AppState>) -> Result<(), String> {
    state.recorder.start().map_err(|err| err.to_string())?;
    processing::emit_status(&app, "recording");
    Ok(())
}

#[tauri::command]
fn stop_recording(app: tauri::AppHandle, state: State<AppState>) -> Result<(), String> {
    let audio = state.recorder.stop().map_err(|err| err.to_string())?;
    processing::emit_status(&app, "transcribing");
    let store = state.settings_store.clone();
    std::thread::spawn(move || {
        if let Err(err) = processing::handle_recording(&app, &store, audio) {
            eprintln!("录音处理失败: {err}");
            processing::emit_status(&app, "error");
        }
    });
    Ok(())
}

#[tauri::command]
fn set_tray_menu(app: tauri::AppHandle, labels: TrayLabels) -> Result<(), String> {
    let show_item = MenuItemBuilder::with_id("show", labels.show_settings)
        .build(&app)
        .map_err(|err| err.to_string())?;
    let quit_item = MenuItemBuilder::with_id("quit", labels.quit)
        .build(&app)
        .map_err(|err| err.to_string())?;
    let menu = MenuBuilder::new(&app)
        .items(&[&show_item, &quit_item])
        .build()
        .map_err(|err| err.to_string())?;

    TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .build(&app)
        .map_err(|err| err.to_string())?;

    Ok(())
}

fn create_status_window(app: &tauri::AppHandle) -> tauri::Result<()> {
    let url = WebviewUrl::App("index.html#status".into());
    let window = WebviewWindowBuilder::new(app, "status", url)
        .title("status")
        .decorations(false)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .focused(false)
        .visible(false)
        .build()?;

    if let Some(monitor) = window.primary_monitor()? {
        let size = monitor.size();
        let work_area = monitor.work_area();
        let width = (size.width as f64 * 0.32).min(420.0).max(240.0);
        let height = 56.0;
        let x = monitor.position().x as f64 + (size.width as f64 - width) / 2.0;
        let y = if cfg!(target_os = "macos") {
            monitor.position().y as f64 + size.height as f64 - height - 12.0
        } else {
            work_area.position.y as f64 + work_area.size.height as f64 - height - 12.0
        };
        let _ = window.set_size(Size::Physical(PhysicalSize::new(width as u32, height as u32)));
        let _ = window.set_position(Position::Physical(PhysicalPosition::new(x as i32, y as i32)));
    }

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_handle = app.handle();
            let store = SettingsStore::new(app_handle.clone());
            app.manage(AppState {
                recorder: RecorderService::new(),
                settings_store: store,
            });

            create_status_window(&app_handle)?;

            if let Some(window) = app.get_webview_window("main") {
                let window_clone = window.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window_clone.hide();
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            update_settings,
            export_settings,
            import_settings,
            start_recording,
            stop_recording,
            set_tray_menu
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

