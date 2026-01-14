mod audio_processing;
mod openai;
mod paste;
mod processing;
mod recorder;
mod settings;
mod status_native;
mod triggers;
mod volcengine;

use recorder::RecorderService;
use settings::{Settings, SettingsStore};
use std::fs;
use std::sync::Mutex;
use tauri::{Manager, State, WindowEvent, Wry};
use tauri::menu::{MenuBuilder, MenuItem, MenuItemBuilder};
use tauri::tray::{TrayIcon, TrayIconBuilder};


#[derive(Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrayLabels {
    show_settings: String,
    quit: String,
}

#[derive(Default)]
struct TrayState {
    tray: Option<TrayIcon<Wry>>,
    show_item: Option<MenuItem<Wry>>,
    quit_item: Option<MenuItem<Wry>>,
}

pub(crate) struct AppState {
    recorder: RecorderService,
    settings_store: SettingsStore,
    tray_state: Mutex<TrayState>,
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
fn start_recording(state: State<AppState>) -> Result<(), String> {
    state.recorder.start().map_err(|err| err.to_string())?;
    processing::emit_status("recording");
    Ok(())
}

#[tauri::command]
fn stop_recording(state: State<AppState>) -> Result<(), String> {
    let audio = state.recorder.stop().map_err(|err| err.to_string())?;
    processing::emit_status("transcribing");
    let store = state.settings_store.clone();
    std::thread::spawn(move || {
        if let Err(err) = processing::handle_recording(&store, audio) {
            eprintln!("录音处理失败: {err}");
            processing::emit_status("error");
        }
    });
    Ok(())
}

#[tauri::command]
fn set_tray_menu(
    app: tauri::AppHandle,
    state: State<AppState>,
    labels: TrayLabels,
) -> Result<(), String> {
    let mut tray_state = state
        .tray_state
        .lock()
        .map_err(|_| "Tray state lock failed".to_string())?;

    if tray_state.tray.is_none() {
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
        let icon = app
            .default_window_icon()
            .ok_or_else(|| "Missing default window icon".to_string())?
            .clone();

        let tray = TrayIconBuilder::new()
            .icon(icon)
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

        tray_state.tray = Some(tray);
        tray_state.show_item = Some(show_item);
        tray_state.quit_item = Some(quit_item);
        return Ok(());
    }

    if let Some(show_item) = tray_state.show_item.as_ref() {
        show_item
            .set_text(labels.show_settings)
            .map_err(|err| err.to_string())?;
    }
    if let Some(quit_item) = tray_state.quit_item.as_ref() {
        quit_item
            .set_text(labels.quit)
            .map_err(|err| err.to_string())?;
    }

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize native status overlay
    if !status_native::init() {
        eprintln!("警告：原生状态窗口初始化失败");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_handle = app.handle();
            let store = SettingsStore::new(app_handle.clone());
            app.manage(AppState {
                recorder: RecorderService::new(),
                settings_store: store,
                tray_state: Mutex::new(TrayState::default()),
            });

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

    // Cleanup native status overlay
    status_native::cleanup();
}

