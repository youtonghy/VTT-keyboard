mod aliyun_realtime;
mod audio_processing;
mod openai;
mod paste;
mod processing;
mod recorder;
mod sensevoice;
mod settings;
mod status_native;
mod transcription;
mod transcription_dispatcher;
mod triggers;
mod updater;
mod util;
mod volcengine;

use recorder::RecorderService;
use sensevoice::model::{
    resolve_vllm_model_id, spec_for_local_model, supports_sherpa_onnx_target, LocalRuntimeKind,
};
use sensevoice::{SenseVoiceManager, SenseVoiceStatus};
use settings::{
    SenseVoiceSettings, Settings, SettingsStore, TranscriptionHistoryItem, TranscriptionProvider,
};
use std::fs;
use std::sync::Mutex;
use tauri::menu::{MenuBuilder, MenuItem, MenuItemBuilder};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent, Wry};
use transcription_dispatcher::TranscriptionDispatcher;
use updater::UpdateManager;

macro_rules! dev_eprintln {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        {
            eprintln!($($arg)*);
        }
    };
}

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
    updater_manager: Mutex<UpdateManager>,
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> Result<Settings, String> {
    state.settings_store.load().map_err(|err| err.to_string())
}

#[tauri::command]
fn update_settings(
    app: tauri::AppHandle,
    state: State<AppState>,
    settings: Settings,
) -> Result<Settings, String> {
    let previous = state
        .settings_store
        .load()
        .map_err(|err| err.to_string())?;
    let previous_local_model = previous.sensevoice.local_model.clone();
    let previous_model_id = previous.sensevoice.model_id.clone();

    let persisted = state
        .settings_store
        .save_user_settings(&settings)
        .map_err(|err| err.to_string())?;

    updater::handle_settings_changed(app.clone(), state.settings_store.clone());

    maybe_restart_local_runtime_if_switched(
        &app,
        &state,
        &previous_local_model,
        &previous_model_id,
        &persisted.sensevoice.local_model,
        &persisted.sensevoice.model_id,
    )?;

    Ok(persisted)
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
    app: tauri::AppHandle,
    state: State<AppState>,
    sensevoice: SenseVoiceSettings,
) -> Result<(), String> {
    let previous = state
        .settings_store
        .load_sensevoice()
        .map_err(|err| err.to_string())?;
    let previous_local_model = previous.local_model.clone();
    let previous_model_id = previous.model_id.clone();

    state
        .settings_store
        .save_sensevoice_editable(&sensevoice)
        .map_err(|err| err.to_string())?;

    maybe_restart_local_runtime_if_switched(
        &app,
        &state,
        &previous_local_model,
        &previous_model_id,
        &sensevoice.local_model,
        &sensevoice.model_id,
    )
}

fn maybe_restart_local_runtime_if_switched(
    app: &tauri::AppHandle,
    state: &State<AppState>,
    previous_local_model: &str,
    previous_model_id: &str,
    next_local_model: &str,
    next_model_id: &str,
) -> Result<(), String> {
    let previous = spec_for_local_model(previous_local_model);
    let next = spec_for_local_model(next_local_model);
    // 比较模型系列和具体变体 ID（解析后的完整 model ID）
    let prev_resolved = resolve_vllm_model_id(previous_local_model, previous_model_id);
    let next_resolved = resolve_vllm_model_id(next_local_model, next_model_id);
    if previous.model_key == next.model_key && prev_resolved == next_resolved {
        return Ok(());
    }

    let mut manager = state
        .sensevoice_manager
        .lock()
        .map_err(|_| "failed to lock SenseVoice manager".to_string())?;

    if !manager.has_running_runtime() {
        return Ok(());
    }

    manager
        .stop_service_force(app, &state.settings_store)
        .map_err(|err| err.to_string())?;
    manager
        .start_service_async(app, &state.settings_store)
        .map_err(|err| err.to_string())?;
    Ok(())
}

#[tauri::command]
fn export_settings(state: State<AppState>, path: String) -> Result<(), String> {
    let settings = state.settings_store.load().map_err(|err| err.to_string())?;
    let data = serde_json::to_string_pretty(&settings).map_err(|err| err.to_string())?;
    fs::write(path, data).map_err(|err| err.to_string())?;
    Ok(())
}

#[tauri::command]
fn import_settings(state: State<AppState>, path: String) -> Result<Settings, String> {
    let data = fs::read_to_string(path).map_err(|err| err.to_string())?;
    let settings: Settings = serde_json::from_str(&data).map_err(|err| err.to_string())?;
    let persisted = state
        .settings_store
        .save_user_settings(&settings)
        .map_err(|err| err.to_string())?;
    Ok(persisted)
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
fn get_transcription_history(
    state: State<AppState>,
) -> Result<Vec<TranscriptionHistoryItem>, String> {
    state
        .settings_store
        .load_transcription_history()
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn clear_transcription_history(state: State<AppState>) -> Result<(), String> {
    state
        .settings_store
        .clear_transcription_history()
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn get_sensevoice_status(state: State<AppState>) -> Result<SenseVoiceStatus, String> {
    let mut manager = state
        .sensevoice_manager
        .lock()
        .map_err(|_| "failed to lock SenseVoice manager".to_string())?;
    manager
        .status(&state.settings_store)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn prepare_sensevoice(
    app: tauri::AppHandle,
    state: State<AppState>,
) -> Result<SenseVoiceStatus, String> {
    let mut manager = state
        .sensevoice_manager
        .lock()
        .map_err(|_| "failed to lock SenseVoice manager".to_string())?;
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
        .map_err(|_| "failed to lock SenseVoice manager".to_string())?;
    manager
        .start_service_async(&app, &state.settings_store)
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
        .map_err(|_| "failed to lock SenseVoice manager".to_string())?;
    manager
        .stop_service(&app, &state.settings_store)
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn update_sensevoice_runtime(
    app: tauri::AppHandle,
    state: State<AppState>,
) -> Result<SenseVoiceStatus, String> {
    let mut manager = state
        .sensevoice_manager
        .lock()
        .map_err(|_| "failed to lock SenseVoice manager".to_string())?;
    manager
        .update_runtime_async(&app, &state.settings_store)
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
        .map_err(|_| "failed to lock tray state".to_string())?;

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
            .ok_or_else(|| "missing default window icon".to_string())?
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
                    let state = app.state::<AppState>();
                    if let Ok(mut manager) = state.sensevoice_manager.lock() {
                        manager.pause_runtime_for_exit(app);
                    }
                    if updater::maybe_install_on_quit(app, &state.settings_store).unwrap_or(false) {
                        return;
                    }
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
fn get_update_status(app: AppHandle) -> Result<updater::UpdateStatusPayload, String> {
    updater::get_status(&app)
}

#[tauri::command]
fn install_downloaded_update(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    if let Ok(mut manager) = state.sensevoice_manager.lock() {
        manager.pause_runtime_for_exit(&app);
    }
    updater::install_downloaded_update(&app)
}

#[tauri::command]
fn dismiss_update_error(app: AppHandle) -> Result<(), String> {
    updater::dismiss_error(&app)
}

#[tauri::command]
fn retry_update_check(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    updater::schedule_update_check(app, state.settings_store.clone(), true);
    Ok(())
}

#[tauri::command]
fn get_app_info() -> serde_json::Value {
    serde_json::json!({
        "buildDate": env!("BUILD_DATE"),
        "platform": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "supportsSherpaOnnxSenseVoice": supports_sherpa_onnx_target(),
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if !status_native::init() {
        dev_eprintln!("warning: failed to initialize native status overlay");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())
                .map_err(|err| err.to_string())?;

            #[cfg(desktop)]
            if let Err(_err) = app.handle().plugin(tauri_plugin_autostart::init(
                tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                Some(vec!["--autostart"]),
            )) {
                dev_eprintln!("failed to initialize autostart plugin: {_err}");
            }

            let app_handle = app.handle();
            let store = SettingsStore::new(app_handle.clone());
            let startup_store = store.clone();
            let startup_app = app_handle.clone();
            let is_autostart_launch = std::env::args().any(|arg| arg == "--autostart");
            let current_version = app.package_info().version.to_string();

            app.manage(AppState {
                recorder: RecorderService::new(),
                transcription_dispatcher: TranscriptionDispatcher::new(
                    app_handle.clone(),
                    store.clone(),
                ),
                settings_store: store,
                sensevoice_manager: Mutex::new(SenseVoiceManager::new()),
                tray_state: Mutex::new(TrayState::default()),
                updater_manager: Mutex::new(UpdateManager::new(current_version)),
            });

            if let Some(window) = app.get_webview_window("main") {
                if is_autostart_launch {
                    let _ = window.hide();
                }
                let window_clone = window.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        dev_eprintln!("close requested, hiding window to tray instead");
                        let _ = window_clone.hide();
                    }
                });
            }

            updater::schedule_update_check(app_handle.clone(), startup_store.clone(), false);

            std::thread::spawn(move || {
                let settings = match startup_store.load() {
                    Ok(value) => value,
                    Err(_err) => {
                        dev_eprintln!("failed to read settings on startup: {_err}");
                        return;
                    }
                };
                if settings.provider != TranscriptionProvider::Sensevoice {
                    return;
                }
                let runtime_kind =
                    spec_for_local_model(&settings.sensevoice.local_model).runtime_kind;
                if !is_autostart_launch
                    && runtime_kind == LocalRuntimeKind::Native
                    && !settings.sensevoice.installed
                {
                    let _ = startup_app.emit("sensevoice-startup-download-required", ());
                    return;
                }
                if !settings.sensevoice.installed {
                    return;
                }
                let state = startup_app.state::<AppState>();
                let Ok(mut manager) = state.sensevoice_manager.lock() else {
                    dev_eprintln!("failed to lock SenseVoice manager on startup");
                    return;
                };
                if let Err(_err) = manager.start_service_async(&startup_app, &state.settings_store)
                {
                    dev_eprintln!("failed to auto start SenseVoice on startup: {_err}");
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
            get_transcription_history,
            clear_transcription_history,
            get_sensevoice_status,
            prepare_sensevoice,
            start_sensevoice_service,
            stop_sensevoice_service,
            update_sensevoice_runtime,
            set_tray_menu,
            get_update_status,
            install_downloaded_update,
            dismiss_update_error,
            retry_update_check,
            get_app_info
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    status_native::cleanup();
}

pub fn parse_sensevoice_worker_job_file_arg(args: &[String]) -> Option<String> {
    sensevoice::worker::parse_job_file_arg(args)
}

pub fn run_sensevoice_worker(job_file: Option<&str>) -> i32 {
    let Some(path) = job_file else {
        dev_eprintln!("missing --job-file <path>");
        return 2;
    };
    sensevoice::worker::run_worker(path)
}
