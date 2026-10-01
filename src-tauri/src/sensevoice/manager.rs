use super::{
    model::{normalize_local_model, spec_for_local_model},
    native_runtime, sherpa, SenseVoiceError,
};
use crate::settings::SettingsStore;
use crate::AppState;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

const RUNTIME_STATE_STOPPED: &str = "stopped";
const RUNTIME_STATE_RUNNING: &str = "running";
const RUNTIME_STATE_STARTING: &str = "starting";

const START_CANCELLED_MARKER: &str = "__sensevoice_start_cancelled__";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SenseVoiceStatus {
    pub installed: bool,
    pub enabled: bool,
    pub running: bool,
    pub runtime_state: String,
    pub local_model: String,
    pub model_id: String,
    pub language: String,
    pub download_state: String,
    pub last_error: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SenseVoiceProgress {
    stage: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    percent: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    downloaded_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

pub struct SenseVoiceManager {
    start_in_progress: bool,
    start_cancel_flag: Arc<AtomicBool>,
    prepare_in_progress: Arc<AtomicBool>,
    /// 标记 pause_runtime_for_exit 是否已执行，避免 Drop 重复操作
    exit_cleanup_done: bool,
}

/// 本地模型根目录（`<app_local_data>/sensevoice/models`），并同步到 sherpa 全局根。
pub fn models_root(app: &AppHandle) -> Result<PathBuf, SenseVoiceError> {
    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|err| SenseVoiceError::Io(err.to_string()))?
        .join("sensevoice")
        .join("models");
    std::fs::create_dir_all(&dir).map_err(|err| SenseVoiceError::Io(err.to_string()))?;
    native_runtime::set_models_root(dir.clone());
    Ok(dir)
}

/// 当前所选模型的文件是否已完整下载（派生自磁盘状态，不依赖持久化标记）
pub fn selected_model_ready(app: &AppHandle, store: &SettingsStore) -> bool {
    let Ok(sensevoice) = store.load_sensevoice() else {
        return false;
    };
    let Ok(root) = models_root(app) else {
        return false;
    };
    sherpa::model_is_ready(&root, spec_for_local_model(&sensevoice.local_model))
}

impl SenseVoiceManager {
    pub fn new() -> Self {
        Self {
            start_in_progress: false,
            start_cancel_flag: Arc::new(AtomicBool::new(false)),
            prepare_in_progress: Arc::new(AtomicBool::new(false)),
            exit_cleanup_done: false,
        }
    }

    pub fn status(
        &mut self,
        app: &AppHandle,
        store: &SettingsStore,
    ) -> Result<SenseVoiceStatus, SenseVoiceError> {
        let sensevoice = store
            .load_sensevoice()
            .map_err(|err| SenseVoiceError::Settings(err.to_string()))?;
        let spec = spec_for_local_model(&sensevoice.local_model);
        let models_root = models_root(app)?;
        let installed = sherpa::model_is_ready(&models_root, spec);
        let loaded = sherpa::is_loaded(&models_root, spec.model_key);
        let running = loaded || self.start_in_progress;
        let runtime_state = if loaded {
            RUNTIME_STATE_RUNNING
        } else if self.start_in_progress {
            RUNTIME_STATE_STARTING
        } else {
            RUNTIME_STATE_STOPPED
        };
        Ok(SenseVoiceStatus {
            installed,
            enabled: sensevoice.enabled,
            running,
            runtime_state: runtime_state.to_string(),
            local_model: spec.model_key.to_string(),
            model_id: sensevoice.model_id,
            language: sensevoice.language,
            download_state: sensevoice.download_state,
            last_error: sensevoice.last_error,
        })
    }

    /// 后台下载并解压所选模型（GitHub Releases 主源，可选 ModelScope 回退）
    pub fn prepare_async(
        &mut self,
        app: &AppHandle,
        store: &SettingsStore,
    ) -> Result<SenseVoiceStatus, SenseVoiceError> {
        if self.prepare_in_progress.load(Ordering::Relaxed) {
            return self.status(app, store);
        }

        let sensevoice = store
            .load_sensevoice()
            .map_err(|err| SenseVoiceError::Settings(err.to_string()))?;
        let local_model = normalize_local_model(&sensevoice.local_model).to_string();
        let models_root = models_root(app)?;

        self.emit_progress(app, "prepare", "Preparing model", Some(5));
        self.update_state(store, "downloading", "", None, None)?;

        self.prepare_in_progress.store(true, Ordering::Relaxed);
        let prepare_flag = Arc::clone(&self.prepare_in_progress);
        let app_handle = app.clone();
        let store_clone = store.clone();
        thread::spawn(move || {
            run_prepare_task(app_handle, store_clone, prepare_flag, local_model, models_root);
        });
        self.status(app, store)
    }

    /// 后台将所选模型加载进内存（进程内 ONNX 推理）
    pub fn start_service_async(
        &mut self,
        app: &AppHandle,
        store: &SettingsStore,
    ) -> Result<SenseVoiceStatus, SenseVoiceError> {
        if self.prepare_in_progress.load(Ordering::Relaxed) {
            return Err(SenseVoiceError::Process(
                "下载任务正在后台执行，请稍后再试".to_string(),
            ));
        }
        let sensevoice = store
            .load_sensevoice()
            .map_err(|err| SenseVoiceError::Settings(err.to_string()))?;
        let spec = spec_for_local_model(&sensevoice.local_model);
        let models_root = models_root(app)?;

        if sherpa::is_loaded(&models_root, spec.model_key) {
            self.start_in_progress = false;
            self.start_cancel_flag.store(false, Ordering::Relaxed);
            return self.status(app, store);
        }
        if self.start_in_progress {
            return self.status(app, store);
        }
        if !sherpa::model_is_ready(&models_root, spec) {
            return Err(SenseVoiceError::Config(format!(
                "{} 尚未下载，请先完成下载",
                spec.display_name
            )));
        }

        self.start_in_progress = true;
        self.start_cancel_flag.store(false, Ordering::Relaxed);

        let app_handle = app.clone();
        let store_clone = store.clone();
        let cancel_flag = Arc::clone(&self.start_cancel_flag);
        let local_model = spec.model_key.to_string();
        let language = sensevoice.language;
        thread::spawn(move || {
            run_load_task(app_handle, store_clone, cancel_flag, local_model, language, models_root);
        });
        self.status(app, store)
    }

    /// 卸载已加载的模型
    pub fn stop_service(
        &mut self,
        app: &AppHandle,
        store: &SettingsStore,
    ) -> Result<SenseVoiceStatus, SenseVoiceError> {
        self.start_cancel_flag.store(true, Ordering::Relaxed);
        self.start_in_progress = false;
        let local_model = store
            .load_sensevoice()
            .map_err(|err| SenseVoiceError::Settings(err.to_string()))?
            .local_model;
        native_runtime::unload(&local_model);
        let _ = self.update_state(store, "ready", "", None, None);
        self.emit_progress(app, "stopped", "Model unloaded", None);
        self.status(app, store)
    }

    /// 卸载模型并删除所选模型的全部文件
    pub fn remove_model(
        &mut self,
        app: &AppHandle,
        store: &SettingsStore,
    ) -> Result<SenseVoiceStatus, SenseVoiceError> {
        self.start_cancel_flag.store(true, Ordering::Relaxed);
        self.start_in_progress = false;
        let local_model = store
            .load_sensevoice()
            .map_err(|err| SenseVoiceError::Settings(err.to_string()))?
            .local_model;
        native_runtime::remove_model_files(&local_model)?;
        let _ = self.update_state(store, "idle", "", Some(false), None);
        self.emit_progress(app, "stopped", "Model files removed", None);
        self.status(app, store)
    }

    /// 应用退出前卸载模型
    pub fn pause_runtime_for_exit(&mut self, app: &AppHandle) {
        self.start_cancel_flag.store(true, Ordering::Relaxed);
        self.start_in_progress = false;
        if sherpa::loaded_model_dir().is_some() {
            sherpa::unload_runtime();
            self.emit_progress(app, "stopped", "Model unloaded", None);
        }
        self.exit_cleanup_done = true;
    }

    pub fn has_running_runtime(&mut self) -> bool {
        self.start_in_progress || sherpa::loaded_model_dir().is_some()
    }

    fn update_state(
        &self,
        store: &SettingsStore,
        download_state: &str,
        last_error: &str,
        installed: Option<bool>,
        enabled: Option<bool>,
    ) -> Result<(), SenseVoiceError> {
        update_state_in_store(store, download_state, last_error, installed, enabled)
    }

    fn emit_progress(&self, app: &AppHandle, stage: &str, message: &str, percent: Option<u8>) {
        emit_progress_payload(app, stage, message, percent, None, None, None);
    }
}

impl Drop for SenseVoiceManager {
    fn drop(&mut self) {
        if !self.exit_cleanup_done && sherpa::loaded_model_dir().is_some() {
            sherpa::unload_runtime();
        }
    }
}

/// 同步确保本地模型已加载：已加载则立即返回，否则触发后台加载并轮询等待。
/// 由转写调度线程在执行前调用，处理应用重启后模型尚未加载的情况。
pub fn ensure_service_ready_blocking(
    app: &AppHandle,
    store: &SettingsStore,
    timeout: Duration,
) -> Result<(), SenseVoiceError> {
    let sensevoice = store
        .load_sensevoice()
        .map_err(|err| SenseVoiceError::Settings(err.to_string()))?;
    if !sensevoice.enabled {
        return Ok(());
    }
    let models_root = models_root(app)?;
    let spec = spec_for_local_model(&sensevoice.local_model);
    if sherpa::is_loaded(&models_root, spec.model_key) {
        return Ok(());
    }
    if !sherpa::model_is_ready(&models_root, spec) {
        // 模型文件缺失：交由上层转写路径给出"未安装"错误
        return Ok(());
    }

    {
        let state = app.state::<AppState>();
        let mut manager = state
            .sensevoice_manager
            .lock()
            .map_err(|_| SenseVoiceError::Process("SenseVoice 状态锁获取失败".to_string()))?;
        manager.start_service_async(app, store)?;
    }

    let started = Instant::now();
    loop {
        if sherpa::is_loaded(&models_root, spec.model_key) {
            return Ok(());
        }
        let current = store
            .load_sensevoice()
            .map_err(|err| SenseVoiceError::Settings(err.to_string()))?;
        if current.download_state.eq_ignore_ascii_case("error") && !current.last_error.is_empty() {
            return Err(SenseVoiceError::Process(current.last_error));
        }
        if started.elapsed() >= timeout {
            return Err(SenseVoiceError::Request(format!(
                "等待本地模型加载超时（{} 秒）",
                timeout.as_secs()
            )));
        }
        thread::sleep(Duration::from_millis(200));
    }
}

fn run_prepare_task(
    app: AppHandle,
    store: SettingsStore,
    prepare_flag: Arc<AtomicBool>,
    local_model: String,
    models_root: PathBuf,
) {
    let result = (|| -> Result<(), SenseVoiceError> {
        emit_progress_payload(
            &app,
            "download",
            "Downloading model",
            Some(0),
            None,
            Some(0),
            None,
        );
        native_runtime::prepare_model(
            &local_model,
            &models_root,
            |line, percent, downloaded_bytes, total_bytes| {
                emit_progress_payload(
                    &app,
                    "download",
                    "Downloading model",
                    percent,
                    Some(line.to_string()),
                    downloaded_bytes,
                    total_bytes,
                );
            },
        )?;
        emit_progress_payload(
            &app,
            "done",
            "Model is ready",
            Some(100),
            Some(format!(
                "{} ready for loading",
                spec_for_local_model(&local_model).display_name
            )),
            None,
            None,
        );
        Ok(())
    })();

    match result {
        Ok(()) => {
            // 仅当用户仍选中该模型时才更新主状态；否则按当前所选模型的磁盘状态回写
            if let Ok(current) = store.load_sensevoice() {
                let still_selected =
                    normalize_local_model(&current.local_model) == local_model.as_str();
                if still_selected {
                    let _ =
                        update_state_in_store(&store, "ready", "", Some(true), Some(true));
                } else {
                    let spec = spec_for_local_model(&current.local_model);
                    let ready = sherpa::model_is_ready(&models_root, spec);
                    let _ = update_state_in_store(
                        &store,
                        if ready { "ready" } else { "idle" },
                        "",
                        Some(ready),
                        None,
                    );
                }
            }
        }
        Err(err) => {
            let message = err.to_string();
            let _ = update_state_in_store(&store, "error", &message, None, None);
            emit_progress_payload(&app, "error", &message, None, None, None, None);
        }
    }
    prepare_flag.store(false, Ordering::Relaxed);
}

fn run_load_task(
    app: AppHandle,
    store: SettingsStore,
    cancel_flag: Arc<AtomicBool>,
    local_model: String,
    language: String,
    models_root: PathBuf,
) {
    let result = (|| -> Result<(), SenseVoiceError> {
        check_start_cancelled(&cancel_flag)?;
        emit_progress_payload(&app, "loading", "Loading model", Some(85), None, None, None);
        update_state_in_store(&store, "loading", "", None, None)?;
        native_runtime::load(&local_model, &models_root, &language)?;
        check_start_cancelled(&cancel_flag)?;
        update_state_in_store(&store, "loaded", "", None, None)?;
        emit_progress_payload(
            &app,
            "done",
            "Model loaded",
            Some(100),
            Some(format!(
                "{} loaded",
                spec_for_local_model(&local_model).display_name
            )),
            None,
            None,
        );
        Ok(())
    })();

    if let Err(err) = result {
        let cancelled =
            is_start_cancelled_error(&err) || cancel_flag.load(Ordering::Relaxed);
        if cancelled {
            // 取消加载：模型文件仍在，回到 ready
            let _ = update_state_in_store(&store, "ready", "", None, None);
            emit_progress_payload(&app, "stopped", "Model load cancelled", None, None, None, None);
        } else {
            let message = err.to_string();
            let _ = update_state_in_store(&store, "error", &message, None, None);
            emit_progress_payload(&app, "error", &message, None, None, None, None);
        }
    }

    cancel_flag.store(false, Ordering::Relaxed);
    if let Ok(mut manager) = app.state::<AppState>().sensevoice_manager.lock() {
        manager.start_in_progress = false;
    }
}

fn check_start_cancelled(cancel_flag: &Arc<AtomicBool>) -> Result<(), SenseVoiceError> {
    if cancel_flag.load(Ordering::Relaxed) {
        return Err(SenseVoiceError::Process(START_CANCELLED_MARKER.to_string()));
    }
    Ok(())
}

fn is_start_cancelled_error(err: &SenseVoiceError) -> bool {
    matches!(err, SenseVoiceError::Process(message) if message == START_CANCELLED_MARKER)
}

fn emit_progress_payload(
    app: &AppHandle,
    stage: &str,
    message: &str,
    percent: Option<u8>,
    detail: Option<String>,
    downloaded_bytes: Option<u64>,
    total_bytes: Option<u64>,
) {
    let payload = SenseVoiceProgress {
        stage: stage.to_string(),
        message: message.to_string(),
        percent,
        downloaded_bytes,
        total_bytes,
        detail,
    };
    let _ = app.emit("sensevoice-progress", payload);
}

fn update_state_in_store(
    store: &SettingsStore,
    download_state: &str,
    last_error: &str,
    installed: Option<bool>,
    enabled: Option<bool>,
) -> Result<(), SenseVoiceError> {
    let mut sensevoice = store
        .load_sensevoice()
        .map_err(|err| SenseVoiceError::Settings(err.to_string()))?;
    sensevoice.download_state = download_state.to_string();
    sensevoice.last_error = last_error.to_string();
    if let Some(next) = installed {
        sensevoice.installed = next;
    }
    if let Some(next) = enabled {
        sensevoice.enabled = next;
    }
    store
        .save_sensevoice(&sensevoice)
        .map_err(|err| SenseVoiceError::Settings(err.to_string()))
}
