mod audio_processing;
mod openai;
mod paste;
mod processing;
mod recorder;
mod sensevoice;
mod settings;
mod status_native;
mod transcription_dispatcher;
mod triggers;
mod volcengine;

use recorder::RecorderService;
use sensevoice::{SenseVoiceManager, SenseVoiceStatus};
use settings::{SenseVoiceSettings, Settings, SettingsStore, TranscriptionProvider};
use std::fs;
use std::sync::Mutex;
use tauri::{Manager, State, WindowEvent, Wry};
use tauri::menu::{MenuBuilder, MenuItem, MenuItemBuilder};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use transcription_dispatcher::TranscriptionDispatcher;


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
    transcription_dispatcher: TranscriptionDispatcher,
    settings_store: SettingsStore,
    sensevoice_manager: Mutex<SenseVoiceManager>,
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
fn get_sensevoice_settings(state: State<AppState>) -> Result<SenseVoiceSettings, String> {
    state
        .settings_store
        .load_sensevoice()
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn update_sensevoice_settings(
    state: State<AppState>,
    sensevoice: SenseVoiceSettings,
) -> Result<(), String> {
    state
        .settings_store
        .save_sensevoice(&sensevoice)
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
    state.transcription_dispatcher.enqueue(audio)?;
    Ok(())
}

#[tauri::command]
fn get_sensevoice_status(state: State<AppState>) -> Result<SenseVoiceStatus, String> {
    let mut manager = state
        .sensevoice_manager
        .lock()
        .map_err(|_| "SenseVoice 状态锁获取失败".to_string())?;
    manager
        .status(&state.settings_store)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn prepare_sensevoice(app: tauri::AppHandle, state: State<AppState>) -> Result<SenseVoiceStatus, String> {
    let mut manager = state
        .sensevoice_manager
        .lock()
        .map_err(|_| "SenseVoice 状态锁获取失败".to_string())?;
    manager
        .prepare_async(&app, &state.settings_store)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn start_sensevoice_service(
    app: tauri::AppHandle,
    state: State<AppState>,
) -> Result<SenseVoiceStatus, String> {
    let mut manager = state
        .sensevoice_manager
        .lock()
        .map_err(|_| "SenseVoice 状态锁获取失败".to_string())?;
    manager
        .start_service(&app, &state.settings_store)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn stop_sensevoice_service(
    app: tauri::AppHandle,
    state: State<AppState>,
) -> Result<SenseVoiceStatus, String> {
    let mut manager = state
        .sensevoice_manager
        .lock()
        .map_err(|_| "SenseVoice 状态锁获取失败".to_string())?;
    manager
        .stop_service(&app, &state.settings_store)
        .map_err(|err| err.to_string())
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

#[tauri::command]
fn get_app_info() -> serde_json::Value {
    serde_json::json!({
        "buildDate": env!("BUILD_DATE")
    })
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
            let startup_store = store.clone();
            let startup_app = app_handle.clone();
            app.manage(AppState {
                recorder: RecorderService::new(),
                transcription_dispatcher: TranscriptionDispatcher::new(store.clone()),
                settings_store: store,
                sensevoice_manager: Mutex::new(SenseVoiceManager::new()),
                tray_state: Mutex::new(TrayState::default()),
            });

            if let Some(window) = app.get_webview_window("main") {
                let window_clone = window.clone();
                let app_for_close = app_handle.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window_clone.hide();
                        let app_for_stop = app_for_close.clone();
                        std::thread::spawn(move || {
                            let state = app_for_stop.state::<AppState>();
                            let Ok(mut manager) = state.sensevoice_manager.lock() else {
                                eprintln!("关闭窗口时获取 SenseVoice 锁失败");
                                return;
                            };
                            if let Err(err) =
                                manager.stop_service(&app_for_stop, &state.settings_store)
                            {
                                eprintln!("关闭窗口时停止 SenseVoice 服务失败: {err}");
                            }
                        });
                    }
                });
            }

            std::thread::spawn(move || {
                let settings = match startup_store.load() {
                    Ok(value) => value,
                    Err(err) => {
                        eprintln!("应用启动读取设置失败: {err}");
                        return;
                    }
                };
                if settings.provider != TranscriptionProvider::Sensevoice {
                    return;
                }
                if !settings.sensevoice.installed || !settings.sensevoice.enabled {
                    return;
                }
                let state = startup_app.state::<AppState>();
                let Ok(mut manager) = state.sensevoice_manager.lock() else {
                    eprintln!("应用启动时获取 SenseVoice 锁失败");
                    return;
                };
                if let Err(err) = manager.start_service(&startup_app, &state.settings_store) {
                    eprintln!("应用启动自动拉起 SenseVoice 失败: {err}");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            update_settings,
            get_sensevoice_settings,
            update_sensevoice_settings,
            export_settings,
            import_settings,
            start_recording,
            stop_recording,
            get_sensevoice_status,
            prepare_sensevoice,
            start_sensevoice_service,
            stop_sensevoice_service,
            set_tray_menu,
            get_app_info
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    // Cleanup native status overlay
    status_native::cleanup();
}

pub fn parse_sensevoice_worker_job_file_arg(args: &[String]) -> Option<String> {
    sensevoice::worker::parse_job_file_arg(args)
}

pub fn run_sensevoice_worker(job_file: Option<&str>) -> i32 {
    let Some(path) = job_file else {
        eprintln!("missing --job-file <path>");
        return 2;
    };
    sensevoice::worker::run_worker(path)
}
