use super::docker_utils::{
    bind_mount, docker_command, docker_container_running, docker_image_exists,
    get_container_label, hide_window, normalize_log_line, normalize_publish_host,
    parse_host_and_port, read_selected_hub, remove_container_if_exists, run_command_streaming,
    start_container,
};
use super::{
    model::{
        docker_container_name, is_vllm_local_model, legacy_container_names,
        normalize_local_model, resolve_vllm_model_id, runtime_container_name,
        runtime_image_tag, service_start_timeout, spec_for_local_model, LocalRuntimeKind,
        CONTAINER_LABEL_MODEL_ID, CONTAINER_LABEL_MODEL_KEY, LOCAL_MODEL_SENSEVOICE,
        LOCAL_MODEL_VOXTRAL,
    },
    native_runtime, SenseVoiceError,
};
use crate::sensevoice::worker::{WorkerEvent, WorkerJob};
use crate::settings::SettingsStore;
use crate::AppState;
use serde::Serialize;
use serde_json::Value;
use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

use url::Url;

/// 日志流批量 emit 间隔：每 150ms 向前端发送一次聚合日志，避免高频 app.emit()
/// 阻塞 Tauri 主线程（Windows WebView2 通过主线程 dispatch JS 评估）
const LOG_EMIT_INTERVAL_MILLIS: u64 = 150;

const PREPARE_SCRIPT: &str = include_str!("scripts/prepare.py");
const SERVER_SCRIPT: &str = include_str!("scripts/server.py");
const REQUIREMENTS_TXT: &str = include_str!("scripts/requirements.txt");
const DOCKERFILE_TXT: &str = include_str!("scripts/Dockerfile");
const VLLM_ENTRYPOINT_SH: &str = include_str!("scripts/vllm_entrypoint.sh");

const VLLM_CONFIG_DIR_NAME: &str = "vllm-config";

const VLLM_INTERNAL_PORT: u16 = 8000;
const VLLM_REQUIRED_DEVICE: &str = "cuda";
const VLLM_GPU_MEMORY_UTILIZATION: f32 = 0.8;
const VOXTRAL_ATTENTION_BACKEND: &str = "TRITON_ATTN";
const HEALTH_REQUEST_TIMEOUT_SECS: u64 = 2;
const HEALTH_MONITOR_WARN_SECS: u64 = 120;
const HEALTH_MONITOR_INTERVAL_MILLIS: u64 = 1000;
const DOCKER_BUILD_TIMEOUT_SECS: u64 = 40 * 60;
const IMAGE_STAMP_FILE: &str = "image.stamp";
const WORKER_ARG: &str = "--sensevoice-worker";
const WORKER_JOB_FILE_ARG: &str = "--job-file";
const START_CANCELLED_MARKER: &str = "__sensevoice_start_cancelled__";
const STOP_MODE_STOP: &str = "stop";
const STOP_MODE_PAUSE: &str = "pause";
const RUNTIME_STATE_STOPPED: &str = "stopped";
const RUNTIME_STATE_RUNNING: &str = "running";
const RUNTIME_STATE_PAUSED: &str = "paused";
const RUNTIME_STATE_STARTING: &str = "starting";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SenseVoiceStatus {
    pub installed: bool,
    pub enabled: bool,
    pub running: bool,
    pub runtime_state: String,
    pub runtime_kind: String,
    pub supports_pause: bool,
    pub local_model: String,
    pub service_url: String,
    pub model_id: String,
    pub device: String,
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

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SenseVoiceRuntimeLog {
    stream: String,
    line: String,
    ts: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RuntimeState {
    Stopped,
    Running,
    Paused,
    Exited,
}

impl RuntimeState {
    fn as_status_str(self) -> &'static str {
        match self {
            RuntimeState::Stopped => RUNTIME_STATE_STOPPED,
            RuntimeState::Running => RUNTIME_STATE_RUNNING,
            RuntimeState::Paused => RUNTIME_STATE_PAUSED,
            RuntimeState::Exited => RUNTIME_STATE_STOPPED,
        }
    }
}

pub struct SenseVoiceManager {
    container_name: Option<String>,
    log_child: Option<Child>,
    prepare_child: Option<Child>,
    start_in_progress: bool,
    start_cancel_flag: Arc<AtomicBool>,
    native_prepare_in_progress: Arc<AtomicBool>,
    /// 缓存的容器运行状态，由后台健康监测线程异步更新，避免在 Mutex 持有期间调用 docker inspect
    container_running_cache: Arc<AtomicBool>,
    /// 缓存的容器暂停状态，由后台流程更新，避免在 Mutex 持有期间调用 docker inspect
    container_paused_cache: Arc<AtomicBool>,
    /// 标记 pause_runtime_for_exit 是否已执行，避免 Drop 重复操作
    exit_cleanup_done: bool,
}

impl SenseVoiceManager {
    pub fn new() -> Self {
        Self {
            container_name: None,
            log_child: None,
            prepare_child: None,
            start_in_progress: false,
            start_cancel_flag: Arc::new(AtomicBool::new(false)),
            native_prepare_in_progress: Arc::new(AtomicBool::new(false)),
            container_running_cache: Arc::new(AtomicBool::new(false)),
            container_paused_cache: Arc::new(AtomicBool::new(false)),
            exit_cleanup_done: false,
        }
    }

    pub fn status(&mut self, store: &SettingsStore) -> Result<SenseVoiceStatus, SenseVoiceError> {
        self.reconcile_prepare_task();
        let sensevoice = store
            .load_sensevoice()
            .map_err(|err| SenseVoiceError::Settings(err.to_string()))?;
        let local_model_spec = spec_for_local_model(&sensevoice.local_model);
        let runtime_state = if local_model_spec.runtime_kind == LocalRuntimeKind::Native {
            if native_runtime::is_loaded(local_model_spec.model_key) {
                RuntimeState::Running
            } else {
                RuntimeState::Stopped
            }
        } else {
            self.refresh_runtime_state_cache()
        };
        let running = runtime_state == RuntimeState::Running || self.start_in_progress;
        let runtime_state = if self.start_in_progress && runtime_state != RuntimeState::Running {
            RUNTIME_STATE_STARTING.to_string()
        } else {
            runtime_state.as_status_str().to_string()
        };
        Ok(SenseVoiceStatus {
            installed: sensevoice.installed,
            enabled: sensevoice.enabled,
            running,
            runtime_state,
            local_model: sensevoice.local_model,
            service_url: sensevoice.service_url,
            model_id: sensevoice.model_id,
            device: sensevoice.device,
            download_state: sensevoice.download_state,
            last_error: sensevoice.last_error,
            runtime_kind: local_model_spec.runtime_kind.as_str().to_string(),
            supports_pause: local_model_spec.supports_pause,
        })
    }

    pub fn prepare_async(
        &mut self,
        app: &AppHandle,
        store: &SettingsStore,
    ) -> Result<SenseVoiceStatus, SenseVoiceError> {
        self.reconcile_prepare_task();
        if self.is_prepare_running() {
            return self.status(store);
        }

        self.emit_progress(app, "prepare", "Preparing runtime", Some(5));
        self.update_state(store, "preparing", "", None, None)?;
        let sensevoice = store
            .load_sensevoice()
            .map_err(|err| SenseVoiceError::Settings(err.to_string()))?;
        let local_model = normalize_local_model(&sensevoice.local_model);
        let local_model_spec = spec_for_local_model(local_model);
        let paths = ensure_paths(app)?;
        native_runtime::set_models_root(paths.models_dir.clone());
        if local_model_spec.runtime_kind == LocalRuntimeKind::Native {
            self.native_prepare_in_progress
                .store(true, Ordering::Relaxed);
            self.update_state(store, "downloading", "", None, None)?;
            let app_handle = app.clone();
            let store_clone = store.clone();
            let prepare_flag = Arc::clone(&self.native_prepare_in_progress);
            let local_model = local_model.to_string();
            let language = sensevoice.language;
            let models_dir = paths.models_dir.clone();
            thread::spawn(move || {
                run_native_prepare_task(
                    app_handle,
                    store_clone,
                    prepare_flag,
                    local_model,
                    language,
                    models_dir,
                );
            });
            return self.status(store);
        }
        if local_model == LOCAL_MODEL_SENSEVOICE {
            write_runtime_files(&paths)?;
        }
        let job = WorkerJob {
            local_model: local_model.to_string(),
            service_url: sensevoice.service_url,
            model_id: sensevoice.model_id,
            language: sensevoice.language,
            device: sensevoice.device,
            runtime_dir: paths.runtime_dir.to_string_lossy().to_string(),
            models_dir: paths.models_dir.to_string_lossy().to_string(),
            state_file: paths.state_file.to_string_lossy().to_string(),
            image_tag: runtime_image_tag(local_model).to_string(),
            container_name: runtime_container_name(local_model).to_string(),
        };
        self.spawn_prepare_worker(app, store, &job)?;
        self.status(store)
    }

    pub fn start_service_async(
        &mut self,
        app: &AppHandle,
        store: &SettingsStore,
    ) -> Result<SenseVoiceStatus, SenseVoiceError> {
        self.reconcile_prepare_task();
        if self.is_prepare_running() {
            return Err(SenseVoiceError::Process(
                "下载任务正在后台执行，请稍后再试".to_string(),
            ));
        }
        let sensevoice = store
            .load_sensevoice()
            .map_err(|err| SenseVoiceError::Settings(err.to_string()))?;
        let local_model = normalize_local_model(&sensevoice.local_model);
        let local_model_spec = spec_for_local_model(local_model);
        let container_name = docker_container_name();
        let runtime_state = if local_model_spec.runtime_kind == LocalRuntimeKind::Native {
            if native_runtime::is_loaded(local_model) {
                RuntimeState::Running
            } else {
                RuntimeState::Stopped
            }
        } else {
            self.refresh_runtime_state_cache()
        };
        // 仅当容器正在运行且模型匹配时才可直接复用
        if runtime_state == RuntimeState::Running
            && local_model_spec.runtime_kind == LocalRuntimeKind::Docker
        {
            // 优先读取宿主机配置文件，回退到 Docker labels
            let config_dir_result = app
                .path()
                .app_local_data_dir()
                .map(|d| d.join("sensevoice").join("runtime").join(VLLM_CONFIG_DIR_NAME));
            let (loaded_key, loaded_id) = config_dir_result
                .ok()
                .and_then(|dir| read_vllm_config_model(&dir))
                .or_else(|| {
                    let k = get_container_label(container_name, CONTAINER_LABEL_MODEL_KEY);
                    let i = get_container_label(container_name, CONTAINER_LABEL_MODEL_ID);
                    Some((k?, i?))
                })
                .unwrap_or_default();
            let expected_model_id = resolve_vllm_model_id(local_model, &sensevoice.model_id);
            if loaded_key == local_model && loaded_id == expected_model_id {
                self.start_in_progress = false;
                self.start_cancel_flag.store(false, Ordering::Relaxed);
                return self.status(store);
            }
            // 模型不匹配，需要重建容器，不要提前返回
        } else if runtime_state == RuntimeState::Running {
            // 原生运行时已在运行
            self.start_in_progress = false;
            self.start_cancel_flag.store(false, Ordering::Relaxed);
            return self.status(store);
        }
        if self.start_in_progress {
            return self.status(store);
        }
        if !sensevoice.installed {
            return Err(SenseVoiceError::Config(
                "SenseVoice 尚未安装，请先完成下载".to_string(),
            ));
        }

        self.start_in_progress = true;
        self.start_cancel_flag.store(false, Ordering::Relaxed);

        let app_handle = app.clone();
        let store_clone = store.clone();
        let cancel_flag = Arc::clone(&self.start_cancel_flag);
        let local_model = local_model.to_string();
        // 检测当前模型容器的精确状态用于启动策略选择
        let container_state = if local_model_spec.runtime_kind == LocalRuntimeKind::Docker {
            docker_container_state(runtime_container_name(&local_model))
                .unwrap_or(RuntimeState::Stopped)
        } else {
            RuntimeState::Stopped
        };
        thread::spawn(move || {
            run_startup_task(
                app_handle,
                store_clone,
                cancel_flag,
                local_model,
                container_state,
            );
        });

        self.status(store)
    }

    pub fn stop_service(
        &mut self,
        app: &AppHandle,
        store: &SettingsStore,
    ) -> Result<SenseVoiceStatus, SenseVoiceError> {
        // 始终使用 pause 模式停止容器（保留容器以便下次快速恢复）
        self.stop_service_with_mode(app, store, STOP_MODE_PAUSE)
    }

    pub fn stop_service_force(
        &mut self,
        app: &AppHandle,
        store: &SettingsStore,
    ) -> Result<SenseVoiceStatus, SenseVoiceError> {
        self.stop_service_with_mode(app, store, STOP_MODE_STOP)
    }

    /// 更新运行时：停止并删除当前模型的容器，更新镜像，然后重新启动
    pub fn update_runtime_async(
        &mut self,
        app: &AppHandle,
        store: &SettingsStore,
    ) -> Result<SenseVoiceStatus, SenseVoiceError> {
        let sensevoice = store
            .load_sensevoice()
            .map_err(|err| SenseVoiceError::Settings(err.to_string()))?;
        let local_model = normalize_local_model(&sensevoice.local_model);
        let local_model_spec = spec_for_local_model(local_model);

        if local_model_spec.runtime_kind == LocalRuntimeKind::Native {
            return Err(SenseVoiceError::Config(
                "原生运行时不支持容器更新操作".to_string(),
            ));
        }

        // 先停止当前服务
        self.stop_service_with_mode(app, store, STOP_MODE_STOP)?;

        let container_name = runtime_container_name(local_model);
        let image_tag = runtime_image_tag(local_model);

        // 删除容器
        let _ = remove_container_if_exists(container_name);

        if is_vllm_local_model(local_model) {
            // vLLM 模型：拉取最新镜像
            self.emit_progress(app, "updating", "Pulling latest vLLM image", Some(30));
            let mut pull = docker_command();
            pull.arg("pull").arg(image_tag);
            run_command_streaming(
                &mut pull,
                "拉取 vLLM Docker 镜像",
                Duration::from_secs(DOCKER_BUILD_TIMEOUT_SECS),
                |line| {
                    let detail = normalize_log_line(line);
                    if !detail.is_empty() {
                        let _ = app.emit(
                            "sensevoice-progress",
                            SenseVoiceProgress {
                                stage: "updating".to_string(),
                                message: "Pulling latest vLLM image".to_string(),
                                percent: Some(50),
                                downloaded_bytes: None,
                                total_bytes: None,
                                detail: Some(detail),
                            },
                        );
                    }
                },
            )
            .map_err(SenseVoiceError::Process)?;
        } else {
            // SenseVoice 模型：删除 image stamp 以触发重建
            let paths = ensure_paths(app)?;
            let stamp_path = paths.runtime_dir.join(IMAGE_STAMP_FILE);
            let _ = fs::remove_file(&stamp_path);
            // 删除旧镜像
            let mut rmi = docker_command();
            rmi.arg("rmi").arg(image_tag);
            hide_window(&mut rmi);
            let _ = rmi.output();
        }

        self.emit_progress(app, "updating", "Runtime updated, restarting", Some(80));

        // 重新启动服务
        self.start_service_async(app, store)
    }

    pub fn pause_runtime_for_exit(&mut self, app: &AppHandle) {
        self.stop_prepare_task();
        self.start_cancel_flag.store(true, Ordering::Relaxed);
        self.start_in_progress = false;
        self.stop_log_stream();
        if native_runtime::is_loaded("sherpa-onnx-sensevoice") {
            native_runtime::unload("sherpa-onnx-sensevoice");
            self.container_running_cache.store(false, Ordering::Relaxed);
            self.container_paused_cache.store(false, Ordering::Relaxed);
            self.container_name = None;
            self.emit_progress(app, "stopped", "Native model unloaded", None);
            self.exit_cleanup_done = true;
            return;
        }
        // 应用退出时 pause 容器（保留状态，下次秒恢复）
        let container_name = docker_container_name();
        if pause_runtime_container_if_needed(container_name).unwrap_or(false) {
            self.container_running_cache.store(false, Ordering::Relaxed);
            self.container_paused_cache.store(true, Ordering::Relaxed);
            self.emit_progress(app, "paused", "Runtime container paused for exit", None);
        } else {
            // 容器可能已停止或不存在
            self.container_running_cache.store(false, Ordering::Relaxed);
            self.container_paused_cache.store(false, Ordering::Relaxed);
            self.emit_progress(app, "stopped", "Runtime container stopped", None);
        }
        self.container_name = None;
        self.exit_cleanup_done = true;
    }

    fn stop_service_with_mode(
        &mut self,
        app: &AppHandle,
        store: &SettingsStore,
        stop_mode: &str,
    ) -> Result<SenseVoiceStatus, SenseVoiceError> {
        self.stop_prepare_task();
        self.start_cancel_flag.store(true, Ordering::Relaxed);
        self.start_in_progress = false;
        self.stop_log_stream();
        let local_model = store
            .load_sensevoice()
            .map_err(|err| SenseVoiceError::Settings(err.to_string()))?
            .local_model;
        let local_model_spec = spec_for_local_model(&local_model);
        if local_model_spec.runtime_kind == LocalRuntimeKind::Native {
            native_runtime::unload(local_model_spec.model_key);
            self.container_running_cache.store(false, Ordering::Relaxed);
            self.container_paused_cache.store(false, Ordering::Relaxed);
            self.container_name = None;
            let _ = self.update_state(store, "ready", "", None, None);
            self.emit_progress(app, "stopped", "Native model unloaded", None);
            return self.status(store);
        }
        let container_name = runtime_container_name(&local_model);
        if stop_mode == STOP_MODE_PAUSE {
            if pause_runtime_container_if_needed(container_name).unwrap_or(false) {
                self.container_running_cache.store(false, Ordering::Relaxed);
                self.container_paused_cache.store(true, Ordering::Relaxed);
                self.container_name = Some(container_name.to_string());
                let _ = self.update_state(store, "paused", "", None, None);
                self.emit_progress(app, "paused", "Service paused", None);
            } else {
                self.container_running_cache.store(false, Ordering::Relaxed);
                self.container_paused_cache.store(false, Ordering::Relaxed);
                self.container_name = None;
                let _ = self.update_state(store, "idle", "", None, None);
                self.emit_progress(app, "stopped", "Service stopped", None);
            }
        } else {
            self.container_running_cache.store(false, Ordering::Relaxed);
            self.container_paused_cache.store(false, Ordering::Relaxed);
            // stop 模式：停止容器但不删除，保留容器以便后续 docker start 恢复
            let _ = stop_container(container_name);
            self.container_name = None;
            let _ = self.update_state(store, "idle", "", None, None);
            self.emit_progress(app, "stopped", "Service stopped", None);
        }
        self.status(store)
    }

    fn refresh_runtime_state_cache(&self) -> RuntimeState {
        let container_name = docker_container_name();
        let runtime_state =
            docker_container_state(container_name).unwrap_or(RuntimeState::Stopped);
        self.container_running_cache
            .store(runtime_state == RuntimeState::Running, Ordering::Relaxed);
        self.container_paused_cache
            .store(runtime_state == RuntimeState::Paused, Ordering::Relaxed);
        runtime_state
    }

    /// 实际检查容器状态（调用 docker inspect，用于后台主动轮询）
    /// 当前由 spawn_health_monitor 后台线程直接调用 docker_container_running() 并更新缓存；
    /// 此方法保留供未来需要主动同步检查的场景使用。
    #[allow(dead_code)]
    fn is_running(&mut self) -> bool {
        self.reconcile_prepare_task();
        let container_name = docker_container_name();
        let runtime_state =
            docker_container_state(container_name).unwrap_or(RuntimeState::Stopped);
        self.container_running_cache
            .store(runtime_state == RuntimeState::Running, Ordering::Relaxed);
        self.container_paused_cache
            .store(runtime_state == RuntimeState::Paused, Ordering::Relaxed);
        if runtime_state != RuntimeState::Stopped && runtime_state != RuntimeState::Exited {
            self.container_name = Some(container_name.to_string());
        } else {
            self.container_name = None;
            self.stop_log_stream();
        }
        runtime_state == RuntimeState::Running
    }

    pub fn has_running_runtime(&mut self) -> bool {
        self.reconcile_prepare_task();
        if self.start_in_progress {
            return true;
        }
        if native_runtime::is_loaded("sherpa-onnx-sensevoice") {
            return true;
        }
        let runtime_state = self.refresh_runtime_state_cache();
        runtime_state == RuntimeState::Running || runtime_state == RuntimeState::Paused
    }

    fn is_prepare_running(&mut self) -> bool {
        self.reconcile_prepare_task();
        self.prepare_child.is_some() || self.native_prepare_in_progress.load(Ordering::Relaxed)
    }

    fn start_log_stream(
        &mut self,
        app: AppHandle,
        container_name: &str,
        log_path: &Path,
        runtime_tail: Arc<Mutex<VecDeque<String>>>,
        startup_completed: Arc<AtomicBool>,
    ) -> Result<(), SenseVoiceError> {
        self.stop_log_stream();

        let mut log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .map_err(|err| SenseVoiceError::Io(format!("打开 SenseVoice 日志失败: {err}")))?;
        let _ = writeln!(log_file, "\n=== sensevoice docker service start ===");
        let stdout_file = log_file
            .try_clone()
            .map_err(|err| SenseVoiceError::Io(format!("复制日志句柄失败: {err}")))?;

        let mut command = docker_command();
        command
            .arg("logs")
            .arg("-f")
            .arg(container_name)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        hide_window(&mut command);

        let mut child = command
            .spawn()
            .map_err(|err| SenseVoiceError::Process(err.to_string()))?;
        attach_runtime_logs(
            &mut child,
            app,
            log_path,
            runtime_tail,
            startup_completed,
            stdout_file,
            log_file,
        )?;
        self.log_child = Some(child);
        Ok(())
    }

    fn stop_log_stream(&mut self) {
        if let Some(mut child) = self.log_child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn spawn_prepare_worker(
        &mut self,
        app: &AppHandle,
        store: &SettingsStore,
        job: &WorkerJob,
    ) -> Result<(), SenseVoiceError> {
        let runtime_dir = Path::new(&job.runtime_dir);
        let jobs_dir = runtime_dir.join("jobs");
        fs::create_dir_all(&jobs_dir).map_err(|err| SenseVoiceError::Io(err.to_string()))?;
        let job_file = jobs_dir.join(format!("job-{}.json", current_timestamp_ms()));
        let content = serde_json::to_string_pretty(job)
            .map_err(|err| SenseVoiceError::Process(err.to_string()))?;
        fs::write(&job_file, content).map_err(|err| SenseVoiceError::Io(err.to_string()))?;

        let current_exe =
            std::env::current_exe().map_err(|err| SenseVoiceError::Process(err.to_string()))?;
        let mut command = Command::new(current_exe);
        command
            .arg(WORKER_ARG)
            .arg(WORKER_JOB_FILE_ARG)
            .arg(&job_file)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        hide_window(&mut command);

        let mut child = command
            .spawn()
            .map_err(|err| SenseVoiceError::Process(format!("启动后台下载任务失败: {err}")))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| SenseVoiceError::Process("后台任务无法读取 stdout".to_string()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| SenseVoiceError::Process("后台任务无法读取 stderr".to_string()))?;

        let app_handle = app.clone();
        let store_clone = store.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let Ok(line) = line else {
                    continue;
                };
                let content = normalize_log_line(&line);
                if content.is_empty() {
                    continue;
                }
                if let Ok(event) = serde_json::from_str::<WorkerEvent>(&content) {
                    handle_worker_event(&app_handle, &store_clone, event);
                } else {
                    emit_progress_payload(
                        &app_handle,
                        "prepare",
                        "Preparing runtime",
                        None,
                        Some(content),
                        None,
                        None,
                    );
                }
            }
        });

        let app_handle = app.clone();
        let store_clone = store.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                let Ok(raw) = line else {
                    continue;
                };
                let content = normalize_log_line(&raw);
                if content.is_empty() {
                    continue;
                }
                if let Ok(event) = serde_json::from_str::<WorkerEvent>(&content) {
                    handle_worker_event(&app_handle, &store_clone, event);
                    continue;
                }
                emit_progress_payload(
                    &app_handle,
                    "prepare",
                    "Preparing runtime",
                    None,
                    Some(content.clone()),
                    None,
                    None,
                );
                let _ = app_handle.emit(
                    "sensevoice-runtime-log",
                    SenseVoiceRuntimeLog {
                        stream: "stderr".to_string(),
                        line: format!("[worker] {content}"),
                        ts: current_timestamp_ms(),
                    },
                );
            }
        });

        self.prepare_child = Some(child);
        Ok(())
    }

    fn reconcile_prepare_task(&mut self) {
        let mut clear = false;
        if let Some(child) = self.prepare_child.as_mut() {
            match child.try_wait() {
                Ok(Some(_)) => clear = true,
                Ok(None) => {}
                Err(_) => clear = true,
            }
        }
        if clear {
            self.prepare_child = None;
        }
    }

    fn stop_prepare_task(&mut self) {
        if let Some(mut child) = self.prepare_child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn update_state(
        &self,
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

    fn emit_progress(&self, app: &AppHandle, stage: &str, message: &str, percent: Option<u8>) {
        self.emit_progress_detail(app, stage, message, percent, None, None, None);
    }

    fn emit_progress_detail(
        &self,
        app: &AppHandle,
        stage: &str,
        message: &str,
        percent: Option<u8>,
        detail: Option<&str>,
        downloaded_bytes: Option<u64>,
        total_bytes: Option<u64>,
    ) {
        let payload = SenseVoiceProgress {
            stage: stage.to_string(),
            message: message.to_string(),
            percent,
            downloaded_bytes,
            total_bytes,
            detail: detail.map(str::to_string),
        };
        let _ = app.emit("sensevoice-progress", payload);
    }
}

impl Drop for SenseVoiceManager {
    fn drop(&mut self) {
        self.stop_prepare_task();
        self.stop_log_stream();
        // 如果 pause_runtime_for_exit 已执行过退出清理，跳过重复操作
        if self.exit_cleanup_done {
            return;
        }
        if native_runtime::is_loaded("sherpa-onnx-sensevoice") {
            native_runtime::unload("sherpa-onnx-sensevoice");
        } else {
            // Drop 时 pause 容器（与退出行为一致）
            let container_name = docker_container_name();
            let _ = pause_runtime_container_if_needed(container_name);
        }
    }
}

fn run_native_prepare_task(
    app: AppHandle,
    store: SettingsStore,
    prepare_flag: Arc<AtomicBool>,
    local_model: String,
    _language: String,
    models_dir: PathBuf,
) {
    let result = (|| -> Result<(), SenseVoiceError> {
        emit_progress_payload(
            &app,
            "download",
            "Downloading native model",
            Some(0),
            None,
            Some(0),
            None,
        );
        native_runtime::prepare_model(
            &local_model,
            &models_dir,
            |line, percent, downloaded_bytes, total_bytes| {
                emit_progress_payload(
                    &app,
                    "download",
                    "Downloading native model",
                    percent,
                    Some(line.to_string()),
                    downloaded_bytes,
                    total_bytes,
                );
            },
        )?;
        update_state_in_store(&store, "ready", "", Some(true), Some(true))?;
        emit_progress_payload(
            &app,
            "done",
            "Native model is ready",
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

    if let Err(err) = result {
        let message = err.to_string();
        let _ = update_state_in_store(&store, "error", &message, None, None);
        emit_progress_payload(&app, "error", &message, None, None, None, None);
    }
    prepare_flag.store(false, Ordering::Relaxed);
}

fn run_startup_task(
    app: AppHandle,
    store: SettingsStore,
    cancel_flag: Arc<AtomicBool>,
    local_model: String,
    container_state: RuntimeState,
) {
    let mut runtime_tail: Option<Arc<Mutex<VecDeque<String>>>> = None;
    let mut log_path: Option<PathBuf> = None;

    let result = (|| -> Result<
        (
            String,
            String,
            PathBuf,
            Arc<Mutex<VecDeque<String>>>,
            Arc<AtomicBool>,
            Arc<AtomicBool>,
        ),
        SenseVoiceError,
    > {
            check_start_cancelled(&cancel_flag)?;
            let local_model = normalize_local_model(&local_model);
            let local_model_spec = spec_for_local_model(local_model);
            let container_name = runtime_container_name(local_model);
            let mut sensevoice = store
                .load_sensevoice()
                .map_err(|err| SenseVoiceError::Settings(err.to_string()))?;
            if is_vllm_local_model(local_model) && sensevoice.device != VLLM_REQUIRED_DEVICE {
                sensevoice.device = VLLM_REQUIRED_DEVICE.to_string();
                store
                    .save_sensevoice(&sensevoice)
                    .map_err(|err| SenseVoiceError::Settings(err.to_string()))?;
            }
            if !sensevoice.installed {
                return Err(SenseVoiceError::Config(
                    "SenseVoice 尚未安装，请先完成下载".to_string(),
                ));
            }

            let paths = ensure_paths(&app)?;
            native_runtime::set_models_root(paths.models_dir.clone());
            if local_model == LOCAL_MODEL_SENSEVOICE {
                write_runtime_files(&paths)?;
            }
            check_start_cancelled(&cancel_flag)?;
            if local_model_spec.runtime_kind == LocalRuntimeKind::Native {
                emit_progress_payload(
                    &app,
                    "loading",
                    "Loading native model",
                    Some(85),
                    None,
                    None,
                    None,
                );
                update_state_in_store(&store, "loading", "", None, None)?;
                native_runtime::load(local_model, &paths.models_dir, &sensevoice.language)?;
                update_state_in_store(&store, "loaded", "", None, None)?;
                emit_progress_payload(
                    &app,
                    "done",
                    "Native model loaded",
                    Some(100),
                    Some(format!("{} loaded", local_model_spec.display_name)),
                    None,
                    None,
                );
                let state = app.state::<AppState>();
                let manager = state.sensevoice_manager.lock().map_err(|_| {
                    SenseVoiceError::Process("SenseVoice 状态锁获取失败".to_string())
                })?;
                return Ok((
                    sensevoice.service_url,
                    local_model.to_string(),
                    paths.runtime_dir.join("server.log"),
                    Arc::new(Mutex::new(VecDeque::new())),
                    Arc::clone(&manager.container_running_cache),
                    Arc::clone(&manager.container_paused_cache),
                ));
            }
            ensure_docker_available()?;
            check_start_cancelled(&cancel_flag)?;
            let (host, port) = parse_host_and_port(&sensevoice.service_url)
                .map_err(SenseVoiceError::Url)?;
            let publish_host = normalize_publish_host(&host)
                .map_err(SenseVoiceError::Config)?;
            let current_log_path = paths.runtime_dir.join("server.log");
            log_path = Some(current_log_path.clone());
            let current_runtime_tail = Arc::new(Mutex::new(VecDeque::with_capacity(200)));
            runtime_tail = Some(Arc::clone(&current_runtime_tail));
            let startup_completed = Arc::new(AtomicBool::new(false));
            let running_cache;
            let paused_cache;
            {
                let state = app.state::<AppState>();
                let manager = state.sensevoice_manager.lock().map_err(|_| {
                    SenseVoiceError::Process("SenseVoice 状态锁获取失败".to_string())
                })?;
                running_cache = Arc::clone(&manager.container_running_cache);
                paused_cache = Arc::clone(&manager.container_paused_cache);
            }

            // 单容器模式：检查当前容器配置的模型是否匹配（含具体变体 ID）
            // 优先读取宿主机配置文件（容器内模型调度的真实来源），回退到 Docker labels
            let mut effective_container_state = container_state;
            let expected_model_id = resolve_vllm_model_id(local_model, &sensevoice.model_id);
            let config_dir = vllm_config_dir(&paths.runtime_dir);
            if container_state != RuntimeState::Stopped {
                let (loaded_key, loaded_id) = read_vllm_config_model(&config_dir)
                    .or_else(|| {
                        // 旧容器没有配置文件，回退到 Docker labels
                        let k = get_container_label(container_name, CONTAINER_LABEL_MODEL_KEY);
                        let i = get_container_label(container_name, CONTAINER_LABEL_MODEL_ID);
                        Some((k?, i?))
                    })
                    .unwrap_or_default();
                let key_matches = loaded_key == local_model;
                let id_matches = loaded_id == expected_model_id;
                if !key_matches || !id_matches {
                    // 模型不匹配：判断是否可以容器内切换
                    let old_is_vllm = is_vllm_local_model(&loaded_key);
                    let new_is_vllm = is_vllm_local_model(local_model);
                    let has_config_file = config_dir.join("model.conf").exists();
                    if old_is_vllm && new_is_vllm && has_config_file {
                        // 同为 vLLM 模型且有 entrypoint：容器内切换（stop → 改配置 → start）
                        emit_progress_payload(
                            &app,
                            "switching",
                            "Switching vLLM model in-place",
                            Some(70),
                            None,
                            None,
                            None,
                        );
                        // 如果容器被 pause 了，先 unpause 再 stop
                        if container_state == RuntimeState::Paused {
                            let _ = unpause_container(container_name);
                        }
                        if container_state == RuntimeState::Running
                            || container_state == RuntimeState::Paused
                        {
                            let _ = stop_container(container_name);
                        }
                        // 写入新配置
                        write_vllm_config(
                            &config_dir,
                            normalize_local_model(local_model),
                            &expected_model_id,
                            port,
                            VLLM_GPU_MEMORY_UTILIZATION,
                            &vllm_extra_args(local_model),
                        )?;
                        // 容器已处于 exited 状态，走 docker start 路径
                        effective_container_state = RuntimeState::Exited;
                    } else {
                        // 不同镜像类型或旧容器无 entrypoint：删除并重建
                        let _ = remove_container_if_exists(container_name);
                        effective_container_state = RuntimeState::Stopped;
                    }
                }
            }

            // 清理旧版多容器遗留（仅在 flag 文件不存在时执行一次）
            cleanup_legacy_containers(&app);

            // 根据容器当前状态选择启动策略
            match effective_container_state {
                RuntimeState::Paused => {
                    // 暂停的容器 → docker unpause（最快路径）
                    emit_progress_payload(
                        &app,
                        "resuming",
                        "Resuming paused local runtime",
                        Some(78),
                        None,
                        None,
                        None,
                    );
                    match unpause_runtime_container_if_needed(container_name) {
                        Ok(true) => {
                            check_start_cancelled(&cancel_flag)?;
                            {
                                let state = app.state::<AppState>();
                                let mut manager =
                                    state.sensevoice_manager.lock().map_err(|_| {
                                        SenseVoiceError::Process(
                                            "SenseVoice 状态锁获取失败".to_string(),
                                        )
                                    })?;
                                manager
                                    .container_running_cache
                                    .store(true, Ordering::Relaxed);
                                manager
                                    .container_paused_cache
                                    .store(false, Ordering::Relaxed);
                                manager.start_log_stream(
                                    app.clone(),
                                    container_name,
                                    &current_log_path,
                                    Arc::clone(&current_runtime_tail),
                                    Arc::clone(&startup_completed),
                                )?;
                                manager.container_name = Some(container_name.to_string());
                            }
                            wait_service_reachable(
                                container_name,
                                &sensevoice.service_url,
                                service_start_timeout(local_model),
                                &current_log_path,
                                &current_runtime_tail,
                                &cancel_flag,
                            )?;
                            update_state_in_store(&store, "running", "", None, None)?;
                            emit_progress_payload(
                                &app,
                                "verify",
                                "Service resumed, model warming up",
                                Some(85),
                                Some("Paused runtime resumed successfully".to_string()),
                                None,
                                None,
                            );
                            return Ok((
                                sensevoice.service_url,
                                local_model.to_string(),
                                current_log_path,
                                current_runtime_tail,
                                running_cache,
                                paused_cache,
                            ));
                        }
                        Ok(false) => {
                            // 容器不存在，走全新创建流程
                        }
                        Err(err) => {
                            emit_progress_payload(
                                &app,
                                "resuming",
                                "Resume failed, fallback to cold startup",
                                Some(80),
                                Some(err.to_string()),
                                None,
                                None,
                            );
                        }
                    }
                }
                RuntimeState::Exited => {
                    // 已停止但存在的容器 → docker start（保留之前的配置）
                    emit_progress_payload(
                        &app,
                        "resuming",
                        "Restarting stopped container",
                        Some(78),
                        None,
                        None,
                        None,
                    );
                    match start_container(container_name) {
                        Ok(()) => {
                            check_start_cancelled(&cancel_flag)?;
                            {
                                let state = app.state::<AppState>();
                                let mut manager =
                                    state.sensevoice_manager.lock().map_err(|_| {
                                        SenseVoiceError::Process(
                                            "SenseVoice 状态锁获取失败".to_string(),
                                        )
                                    })?;
                                manager
                                    .container_running_cache
                                    .store(true, Ordering::Relaxed);
                                manager
                                    .container_paused_cache
                                    .store(false, Ordering::Relaxed);
                                manager.start_log_stream(
                                    app.clone(),
                                    container_name,
                                    &current_log_path,
                                    Arc::clone(&current_runtime_tail),
                                    Arc::clone(&startup_completed),
                                )?;
                                manager.container_name = Some(container_name.to_string());
                            }
                            wait_service_reachable(
                                container_name,
                                &sensevoice.service_url,
                                service_start_timeout(local_model),
                                &current_log_path,
                                &current_runtime_tail,
                                &cancel_flag,
                            )?;
                            update_state_in_store(&store, "running", "", None, None)?;
                            emit_progress_payload(
                                &app,
                                "verify",
                                "Service restarted, model warming up",
                                Some(85),
                                Some("Existing container restarted".to_string()),
                                None,
                                None,
                            );
                            return Ok((
                                sensevoice.service_url,
                                local_model.to_string(),
                                current_log_path,
                                current_runtime_tail,
                                running_cache,
                                paused_cache,
                            ));
                        }
                        Err(err) => {
                            emit_progress_payload(
                                &app,
                                "resuming",
                                "Container restart failed, recreating",
                                Some(80),
                                Some(err.to_string()),
                                None,
                                None,
                            );
                            // 清理坏容器，走全新创建
                            let _ = remove_container_if_exists(container_name);
                        }
                    }
                }
                RuntimeState::Running => {
                    // 已经在运行（不应该到这里，但防御性处理）
                    check_start_cancelled(&cancel_flag)?;
                    {
                        let state = app.state::<AppState>();
                        let mut manager = state.sensevoice_manager.lock().map_err(|_| {
                            SenseVoiceError::Process("SenseVoice 状态锁获取失败".to_string())
                        })?;
                        manager
                            .container_running_cache
                            .store(true, Ordering::Relaxed);
                        manager
                            .container_paused_cache
                            .store(false, Ordering::Relaxed);
                        manager.start_log_stream(
                            app.clone(),
                            container_name,
                            &current_log_path,
                            Arc::clone(&current_runtime_tail),
                            Arc::clone(&startup_completed),
                        )?;
                        manager.container_name = Some(container_name.to_string());
                    }
                    wait_service_reachable(
                        container_name,
                        &sensevoice.service_url,
                        service_start_timeout(local_model),
                        &current_log_path,
                        &current_runtime_tail,
                        &cancel_flag,
                    )?;
                    update_state_in_store(&store, "running", "", None, None)?;
                    return Ok((
                        sensevoice.service_url,
                        local_model.to_string(),
                        current_log_path,
                        current_runtime_tail,
                        running_cache,
                        paused_cache,
                    ));
                }
                RuntimeState::Stopped => {
                    // 容器不存在，需要全新创建（下面处理）
                }
            }

            // 全新创建容器
            if local_model == LOCAL_MODEL_SENSEVOICE {
                ensure_runtime_image(&app, &paths.runtime_dir)?;
            } else {
                ensure_vllm_image(&app)?;
            }
            check_start_cancelled(&cancel_flag)?;

            // 确保清理同名旧容器（可能是之前失败残留）
            let _ = remove_container_if_exists(container_name);
            if local_model == LOCAL_MODEL_SENSEVOICE {
                let hub = read_selected_hub(&paths.state_file).unwrap_or_else(|| "hf".to_string());
                run_service_container(
                    container_name,
                    &publish_host,
                    port,
                    &paths.models_dir,
                    &sensevoice.model_id,
                    &sensevoice.device,
                    &hub,
                )?;
            } else {
                let model_id = resolve_vllm_model_id(local_model, &sensevoice.model_id);
                run_vllm_service_container(
                    local_model,
                    container_name,
                    &publish_host,
                    port,
                    &paths.models_dir,
                    &model_id,
                    &config_dir,
                )?;
            }
            check_start_cancelled(&cancel_flag)?;
            {
                let state = app.state::<AppState>();
                let mut manager = state.sensevoice_manager.lock().map_err(|_| {
                    SenseVoiceError::Process("SenseVoice 状态锁获取失败".to_string())
                })?;
                manager
                    .container_running_cache
                    .store(true, Ordering::Relaxed);
                manager
                    .container_paused_cache
                    .store(false, Ordering::Relaxed);
                manager.start_log_stream(
                    app.clone(),
                    container_name,
                    &current_log_path,
                    Arc::clone(&current_runtime_tail),
                    Arc::clone(&startup_completed),
                )?;
                manager.container_name = Some(container_name.to_string());
            }

            wait_service_reachable(
                container_name,
                &sensevoice.service_url,
                service_start_timeout(local_model),
                &current_log_path,
                &current_runtime_tail,
                &cancel_flag,
            )?;
            update_state_in_store(&store, "running", "", None, None)?;
            emit_progress_payload(
                &app,
                "verify",
                "Service started, model warming up",
                Some(85),
                Some("Service is reachable; model warmup is still running".to_string()),
                None,
                None,
            );

            Ok((
                sensevoice.service_url,
                local_model.to_string(),
                current_log_path,
                current_runtime_tail,
                running_cache,
                paused_cache,
            ))
        })();

    match result {
        Ok((
            service_url,
            local_model,
            monitor_log_path,
            monitor_runtime_tail,
            running_cache,
            paused_cache,
        )) => {
            finish_start_task(&app, &cancel_flag);
            if spec_for_local_model(&local_model).runtime_kind == LocalRuntimeKind::Native {
                running_cache.store(true, Ordering::Relaxed);
                paused_cache.store(false, Ordering::Relaxed);
                return;
            }
            spawn_health_monitor(
                app,
                store,
                local_model,
                service_url,
                monitor_log_path,
                monitor_runtime_tail,
                cancel_flag,
                running_cache,
                paused_cache,
            );
        }
        Err(err) => {
            let cancelled = is_start_cancelled_error(&err) || cancel_flag.load(Ordering::Relaxed);
            let local_model_spec = spec_for_local_model(&local_model);
            handle_startup_failure(
                &app,
                &store,
                err,
                cancelled,
                runtime_tail.as_ref(),
                log_path.as_deref(),
                local_model_spec.runtime_kind,
            );
            finish_start_task(&app, &cancel_flag);
        }
    }
}

fn spawn_health_monitor(
    app: AppHandle,
    store: SettingsStore,
    local_model: String,
    service_url: String,
    log_path: PathBuf,
    runtime_tail: Arc<Mutex<VecDeque<String>>>,
    cancel_flag: Arc<AtomicBool>,
    running_cache: Arc<AtomicBool>,
    paused_cache: Arc<AtomicBool>,
) {
    thread::spawn(move || {
        let client = health_client();
        let local_model = normalize_local_model(&local_model).to_string();
        let container_name = runtime_container_name(&local_model).to_string();
        let is_vllm_model = is_vllm_local_model(&local_model);
        let health_url = format!("{}/health", service_url.trim_end_matches('/'));
        let started = Instant::now();
        let mut warned = false;
        let mut last_warmup_emit = Instant::now()
            .checked_sub(Duration::from_secs(5))
            .unwrap_or_else(Instant::now);

        loop {
            if cancel_flag.load(Ordering::Relaxed) {
                running_cache.store(false, Ordering::Relaxed);
                paused_cache.store(false, Ordering::Relaxed);
                // 通知前端进度已终止，清除残留的 verify/warmup 阶段状态
                emit_progress_payload(
                    &app,
                    "stopped",
                    "SenseVoice service stopped",
                    None,
                    None,
                    None,
                    None,
                );
                return;
            }

            match docker_container_running(&container_name) {
                Ok(true) => {
                    // 更新缓存，供 status() / start_service_async() 快速读取
                    running_cache.store(true, Ordering::Relaxed);
                    paused_cache.store(false, Ordering::Relaxed);
                }
                Ok(false) => {
                    running_cache.store(false, Ordering::Relaxed);
                    paused_cache.store(false, Ordering::Relaxed);
                    report_monitor_failure(
                        &app,
                        &store,
                        "SenseVoice 服务容器已退出".to_string(),
                        &runtime_tail,
                        &log_path,
                    );
                    return;
                }
                Err(err) => {
                    running_cache.store(false, Ordering::Relaxed);
                    paused_cache.store(false, Ordering::Relaxed);
                    report_monitor_failure(
                        &app,
                        &store,
                        format!("SenseVoice 服务状态检查失败: {err}"),
                        &runtime_tail,
                        &log_path,
                    );
                    return;
                }
            }

            if let Ok(response) = client.get(&health_url).send() {
                let status = response.status();
                let body = response.text().unwrap_or_default();
                if status.is_success() {
                    if is_service_ready(&client, &service_url, &body, is_vllm_model) {
                        let _ = update_state_in_store(&store, "ready", "", None, None);
                        emit_progress_payload(
                            &app,
                            "done",
                            "SenseVoice service ready",
                            Some(100),
                            None,
                            None,
                            None,
                        );
                        return;
                    }
                    if last_warmup_emit.elapsed() >= Duration::from_secs(3) {
                        emit_progress_payload(
                            &app,
                            "warmup",
                            "SenseVoice model warming up",
                            Some(92),
                            None,
                            None,
                            None,
                        );
                        last_warmup_emit = Instant::now();
                    }
                }
            }

            if !warned && started.elapsed() >= Duration::from_secs(HEALTH_MONITOR_WARN_SECS) {
                emit_progress_payload(
                    &app,
                    "warmup",
                    "SenseVoice model warmup is taking longer than expected",
                    Some(92),
                    Some("Service is available, model is still warming up".to_string()),
                    None,
                    None,
                );
                warned = true;
            }

            thread::sleep(Duration::from_millis(HEALTH_MONITOR_INTERVAL_MILLIS));
        }
    });
}

fn wait_service_reachable(
    container_name: &str,
    service_url: &str,
    timeout: Duration,
    log_path: &Path,
    runtime_tail: &Arc<Mutex<VecDeque<String>>>,
    cancel_flag: &Arc<AtomicBool>,
) -> Result<(), SenseVoiceError> {
    let url = format!("{}/health", service_url.trim_end_matches('/'));
    let client = health_client();
    let started = Instant::now();

    while started.elapsed() < timeout {
        check_start_cancelled(cancel_flag)?;
        match docker_container_running(container_name) {
            Ok(true) => {}
            Ok(false) => {
                let tail = collect_runtime_tail_with_retry(runtime_tail, 30, log_path);
                return Err(SenseVoiceError::Request(format!(
                    "SenseVoice 服务容器已退出。最近日志: {tail}"
                )));
            }
            Err(err) => {
                return Err(SenseVoiceError::Request(format!(
                    "SenseVoice 服务状态检查失败: {err}"
                )));
            }
        }

        // 先尝试直接 HTTP 健康检查
        if let Ok(response) = client.get(&url).send() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            let body_is_json = serde_json::from_str::<Value>(&body).is_ok();
            if status.is_success() || body_is_json {
                return Ok(());
            }
        }

        // HTTP 失败（Docker Desktop/WSL2 端口映射尚未就绪）时，
        // 尝试 docker exec 内部健康检查，绕过端口映射延迟问题
        if let Some(port) = extract_port_from_url(service_url) {
            if docker_exec_health_check(container_name, port) {
                return Ok(());
            }
        }

        thread::sleep(Duration::from_millis(500));
    }

    let tail = collect_runtime_tail_with_retry(runtime_tail, 30, log_path);
    Err(SenseVoiceError::Request(format!(
        "SenseVoice 服务启动超时（{} 秒）。最近日志: {}",
        timeout.as_secs(),
        tail
    )))
}

fn report_monitor_failure(
    app: &AppHandle,
    store: &SettingsStore,
    message: String,
    runtime_tail: &Arc<Mutex<VecDeque<String>>>,
    log_path: &Path,
) {
    let tail = collect_runtime_tail_with_retry(runtime_tail, 30, log_path);
    let full_message = format!("{message}。最近日志: {tail}");
    let _ = update_state_in_store(store, "error", &full_message, None, None);
    emit_progress_payload(
        app,
        "error",
        "SenseVoice health monitor failed",
        None,
        Some(full_message),
        None,
        None,
    );
    cleanup_runtime_state(app);
    let _ = stop_container(docker_container_name());
}

fn handle_startup_failure(
    app: &AppHandle,
    store: &SettingsStore,
    err: SenseVoiceError,
    cancelled: bool,
    runtime_tail: Option<&Arc<Mutex<VecDeque<String>>>>,
    log_path: Option<&Path>,
    runtime_kind: LocalRuntimeKind,
) {
    if cancelled {
        let cancelled_state = if runtime_kind == LocalRuntimeKind::Native {
            "ready"
        } else {
            "idle"
        };
        let _ = update_state_in_store(store, cancelled_state, "", None, None);
    } else {
        let mut message = err.to_string();
        // 若错误消息中已含"最近日志:"（如来自 wait_service_reachable 的超时/退出错误），
        // 则不再重复追加，避免日志内容被拼接两次
        let already_has_tail = message.contains("最近日志:");
        if !already_has_tail {
            if let (Some(tail_store), Some(path)) = (runtime_tail, log_path) {
                let tail = collect_runtime_tail_with_retry(tail_store, 30, path);
                if tail != "（无日志）" {
                    message = format!("{message}。最近日志: {tail}");
                }
            }
        }
        let _ = update_state_in_store(store, "error", &message, None, None);
        emit_progress_payload(
            app,
            "error",
            "SenseVoice startup failed",
            None,
            Some(message),
            None,
            None,
        );
    }

    cleanup_runtime_state(app);
    if runtime_kind == LocalRuntimeKind::Docker {
        // 启动失败时停止容器但保留，便于调试和后续重试
        let _ = stop_container(docker_container_name());
    }
}

fn cleanup_runtime_state(app: &AppHandle) {
    if let Ok(mut manager) = app.state::<AppState>().sensevoice_manager.lock() {
        manager.stop_log_stream();
        manager.container_name = None;
        manager
            .container_running_cache
            .store(false, Ordering::Relaxed);
        manager
            .container_paused_cache
            .store(false, Ordering::Relaxed);
    }
}

fn finish_start_task(app: &AppHandle, cancel_flag: &Arc<AtomicBool>) {
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

fn health_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(HEALTH_REQUEST_TIMEOUT_SECS))
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new())
}

fn is_service_ready(
    client: &reqwest::blocking::Client,
    service_url: &str,
    health_body: &str,
    is_vllm_model: bool,
) -> bool {
    match parse_health_ready_field(health_body) {
        Some(ready) => ready,
        None if is_vllm_model => check_vllm_models_ready(client, service_url),
        None => false,
    }
}

fn parse_health_ready_field(body: &str) -> Option<bool> {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|json| json.get("ready").and_then(|value| value.as_bool()))
}

fn check_vllm_models_ready(client: &reqwest::blocking::Client, service_url: &str) -> bool {
    let models_url = format!("{}/v1/models", service_url.trim_end_matches('/'));
    let Ok(response) = client.get(&models_url).send() else {
        return false;
    };
    if !response.status().is_success() {
        return false;
    }
    let body = response.text().unwrap_or_default();
    parse_vllm_models_response_ready(&body)
}

fn parse_vllm_models_response_ready(body: &str) -> bool {
    serde_json::from_str::<Value>(body)
        .ok()
        .map(|json| {
            json.get("data")
                .and_then(|value| value.as_array())
                .is_some_and(|models| !models.is_empty())
        })
        .unwrap_or(false)
}

fn handle_worker_event(app: &AppHandle, store: &SettingsStore, event: WorkerEvent) {
    match event {
        WorkerEvent::Progress {
            stage,
            message,
            percent,
            downloaded_bytes,
            total_bytes,
            detail,
        } => {
            emit_progress_payload(
                app,
                &stage,
                &message,
                percent,
                detail,
                downloaded_bytes,
                total_bytes,
            );
        }
        WorkerEvent::RuntimeLog { stream, line, .. } => {
            let _ = app.emit(
                "sensevoice-runtime-log",
                SenseVoiceRuntimeLog {
                    stream,
                    line,
                    ts: current_timestamp_ms(),
                },
            );
        }
        WorkerEvent::State {
            download_state,
            last_error,
            installed,
            enabled,
        } => {
            let _ = update_state_in_store(store, &download_state, &last_error, installed, enabled);
        }
        WorkerEvent::Done { message } => {
            emit_progress_payload(
                app,
                "done",
                "SenseVoice service started",
                Some(100),
                Some(message),
                None,
                None,
            );
        }
        WorkerEvent::Error { message } => {
            let _ = update_state_in_store(store, "error", &message, None, None);
            emit_progress_payload(app, "error", &message, None, None, None, None);
        }
    }
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

struct SenseVoicePaths {
    root_dir: PathBuf,
    runtime_dir: PathBuf,
    models_dir: PathBuf,
    state_file: PathBuf,
}

fn ensure_paths(app: &AppHandle) -> Result<SenseVoicePaths, SenseVoiceError> {
    let root = app
        .path()
        .app_local_data_dir()
        .map_err(|err| SenseVoiceError::Io(err.to_string()))?
        .join("sensevoice");
    let runtime_dir = root.join("runtime");
    let models_dir = root.join("models");
    let state_file = root.join("state.json");
    fs::create_dir_all(&runtime_dir).map_err(|err| SenseVoiceError::Io(err.to_string()))?;
    fs::create_dir_all(&models_dir).map_err(|err| SenseVoiceError::Io(err.to_string()))?;
    Ok(SenseVoicePaths {
        root_dir: root,
        runtime_dir,
        models_dir,
        state_file,
    })
}

fn write_runtime_files(paths: &SenseVoicePaths) -> Result<(), SenseVoiceError> {
    fs::create_dir_all(&paths.runtime_dir).map_err(|err| SenseVoiceError::Io(err.to_string()))?;
    fs::create_dir_all(&paths.models_dir).map_err(|err| SenseVoiceError::Io(err.to_string()))?;
    fs::create_dir_all(&paths.root_dir).map_err(|err| SenseVoiceError::Io(err.to_string()))?;

    fs::write(paths.runtime_dir.join("prepare.py"), PREPARE_SCRIPT)
        .map_err(|err| SenseVoiceError::Io(err.to_string()))?;
    fs::write(paths.runtime_dir.join("server.py"), SERVER_SCRIPT)
        .map_err(|err| SenseVoiceError::Io(err.to_string()))?;
    fs::write(paths.runtime_dir.join("requirements.txt"), REQUIREMENTS_TXT)
        .map_err(|err| SenseVoiceError::Io(err.to_string()))?;
    fs::write(paths.runtime_dir.join("Dockerfile"), DOCKERFILE_TXT)
        .map_err(|err| SenseVoiceError::Io(err.to_string()))?;

    if !paths.state_file.exists() {
        fs::write(&paths.state_file, "{}").map_err(|err| SenseVoiceError::Io(err.to_string()))?;
    }
    Ok(())
}

fn ensure_docker_available() -> Result<(), SenseVoiceError> {
    let mut version = docker_command();
    version
        .arg("version")
        .arg("--format")
        .arg("{{.Client.Version}}");
    hide_window(&mut version);
    let output = version.output().map_err(|err| {
        SenseVoiceError::Config(format!("未检测到 Docker，请先安装 Docker: {err}"))
    })?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(SenseVoiceError::Config(format!(
            "Docker 不可用，请先安装 Docker Desktop 并确保 docker 命令可执行: {detail}"
        )));
    }

    let mut info = docker_command();
    info.arg("info");
    hide_window(&mut info);
    let output = info
        .output()
        .map_err(|err| SenseVoiceError::Config(format!("无法连接 Docker daemon: {err}")))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(SenseVoiceError::Config(format!(
            "Docker daemon 未运行，请先启动 Docker Desktop: {detail}"
        )));
    }
    Ok(())
}

fn ensure_runtime_image(app: &AppHandle, runtime_dir: &Path) -> Result<(), SenseVoiceError> {
    let stamp_path = runtime_dir.join(IMAGE_STAMP_FILE);
    let expected_stamp = runtime_stamp();
    let previous_stamp = fs::read_to_string(&stamp_path).unwrap_or_default();
    let image_tag = runtime_image_tag(LOCAL_MODEL_SENSEVOICE);
    let has_image = docker_image_exists(image_tag);

    if has_image && previous_stamp.trim() == expected_stamp {
        return Ok(());
    }

    let payload = SenseVoiceProgress {
        stage: "install".to_string(),
        message: "Building Docker image".to_string(),
        percent: Some(35),
        downloaded_bytes: None,
        total_bytes: None,
        detail: Some(format!("Building image {image_tag}")),
    };
    let _ = app.emit("sensevoice-progress", payload);

    let mut build = docker_command();
    build.arg("build").arg("-t").arg(image_tag).arg(runtime_dir);
    run_command_streaming(
        &mut build,
        "构建 SenseVoice Docker 镜像",
        Duration::from_secs(DOCKER_BUILD_TIMEOUT_SECS),
        |line| {
            let detail = normalize_log_line(line);
            if !detail.is_empty() {
                let payload = SenseVoiceProgress {
                    stage: "install".to_string(),
                    message: "Building Docker image".to_string(),
                    percent: Some(35),
                    downloaded_bytes: None,
                    total_bytes: None,
                    detail: Some(detail),
                };
                let _ = app.emit("sensevoice-progress", payload);
            }
        },
    )
    .map_err(SenseVoiceError::Process)?;

    fs::write(stamp_path, expected_stamp).map_err(|err| SenseVoiceError::Io(err.to_string()))?;
    Ok(())
}

fn ensure_vllm_image(app: &AppHandle) -> Result<(), SenseVoiceError> {
    let image_tag = runtime_image_tag(LOCAL_MODEL_VOXTRAL);
    if docker_image_exists(image_tag) {
        return Ok(());
    }
    let payload = SenseVoiceProgress {
        stage: "install".to_string(),
        message: "Pulling vLLM Docker image".to_string(),
        percent: Some(35),
        downloaded_bytes: None,
        total_bytes: None,
        detail: Some(format!("Pulling image {image_tag}")),
    };
    let _ = app.emit("sensevoice-progress", payload);

    let mut pull = docker_command();
    pull.arg("pull").arg(image_tag);
    run_command_streaming(
        &mut pull,
        "拉取 vLLM Docker 镜像",
        Duration::from_secs(DOCKER_BUILD_TIMEOUT_SECS),
        |line| {
            let detail = normalize_log_line(line);
            if !detail.is_empty() {
                let payload = SenseVoiceProgress {
                    stage: "install".to_string(),
                    message: "Pulling vLLM Docker image".to_string(),
                    percent: Some(35),
                    downloaded_bytes: None,
                    total_bytes: None,
                    detail: Some(detail),
                };
                let _ = app.emit("sensevoice-progress", payload);
            }
        },
    )
    .map_err(SenseVoiceError::Process)?;
    Ok(())
}

fn runtime_stamp() -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    PREPARE_SCRIPT.hash(&mut hasher);
    SERVER_SCRIPT.hash(&mut hasher);
    REQUIREMENTS_TXT.hash(&mut hasher);
    DOCKERFILE_TXT.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

fn run_service_container(
    container_name: &str,
    publish_host: &str,
    port: u16,
    model_dir: &Path,
    model_id: &str,
    device: &str,
    hub: &str,
) -> Result<(), SenseVoiceError> {
    fs::create_dir_all(model_dir).map_err(|err| SenseVoiceError::Io(err.to_string()))?;
    let mut command = docker_command();
    command
        .arg("run")
        .arg("-d")
        .arg("--name")
        .arg(container_name)
        .arg("--label")
        .arg(format!("{CONTAINER_LABEL_MODEL_KEY}={LOCAL_MODEL_SENSEVOICE}"))
        .arg("--label")
        .arg(format!("{CONTAINER_LABEL_MODEL_ID}={model_id}"))
        .arg("-p")
        .arg(format!("{publish_host}:{port}:{port}"))
        .arg("--mount")
        .arg(bind_mount(model_dir, "/models"))
        .arg("-e")
        .arg(format!("SENSEVOICE_MODEL_ID={model_id}"))
        .arg("-e")
        .arg("SENSEVOICE_MODEL_DIR=/models")
        .arg("-e")
        .arg(format!("SENSEVOICE_DEVICE={device}"))
        .arg("-e")
        .arg(format!("SENSEVOICE_HUB={hub}"))
        .arg("-e")
        .arg("SENSEVOICE_HOST=0.0.0.0")
        .arg("-e")
        .arg(format!("SENSEVOICE_PORT={port}"))
        .arg(runtime_image_tag(LOCAL_MODEL_SENSEVOICE));

    hide_window(&mut command);
    let output = command
        .output()
        .map_err(|err| SenseVoiceError::Process(err.to_string()))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let details = if !stderr.is_empty() { stderr } else { stdout };
    Err(SenseVoiceError::Process(format!(
        "启动 SenseVoice 容器失败: {details}"
    )))
}

fn run_vllm_service_container(
    local_model: &str,
    container_name: &str,
    publish_host: &str,
    host_port: u16,
    model_dir: &Path,
    model_id: &str,
    config_dir: &Path,
) -> Result<(), SenseVoiceError> {
    fs::create_dir_all(model_dir).map_err(|err| SenseVoiceError::Io(err.to_string()))?;
    // 写入 entrypoint.sh 和 model.conf 到宿主机配置目录
    write_vllm_config(
        config_dir,
        normalize_local_model(local_model),
        model_id,
        VLLM_INTERNAL_PORT,
        VLLM_GPU_MEMORY_UTILIZATION,
        &vllm_extra_args(local_model),
    )?;
    let mut gpu_command = docker_command();
    gpu_command
        .arg("run")
        .arg("-d")
        .arg("--name")
        .arg(container_name)
        .arg("--label")
        .arg(format!("{CONTAINER_LABEL_MODEL_KEY}={local_model}"))
        .arg("--label")
        .arg(format!("{CONTAINER_LABEL_MODEL_ID}={model_id}"))
        .arg("--runtime")
        .arg("nvidia")
        .arg("--gpus")
        .arg("all")
        .arg("-p")
        .arg(format!("{publish_host}:{host_port}:{VLLM_INTERNAL_PORT}"))
        .arg("--mount")
        .arg(bind_mount(model_dir, "/root/.cache/huggingface"))
        .arg("--mount")
        .arg(bind_mount(config_dir, "/config"))
        .arg("--ipc=host")
        .arg("--entrypoint")
        .arg("/bin/bash")
        .arg(runtime_image_tag(local_model))
        .arg("/config/entrypoint.sh");
    hide_window(&mut gpu_command);
    let gpu_output = gpu_command
        .output()
        .map_err(|err| SenseVoiceError::Process(err.to_string()))?;
    if gpu_output.status.success() {
        return Ok(());
    }

    let _ = remove_container_if_exists(container_name);
    let gpu_error = docker_output_detail(&gpu_output);
    if local_model == LOCAL_MODEL_VOXTRAL {
        return Err(SenseVoiceError::Process(format!(
            "启动 Voxtral 容器失败：Voxtral 仅支持 CUDA GPU，并已禁用 FlashAttention（使用 TRITON_ATTN）。请确认 NVIDIA GPU 与 Docker NVIDIA Runtime 可用。详情: {gpu_error}"
        )));
    }
    Err(SenseVoiceError::Process(format!(
        "启动 Qwen3-ASR 容器失败：Qwen3-ASR 当前通过 vLLM 在 CUDA GPU 上运行。请确认 NVIDIA GPU 与 Docker NVIDIA Runtime 可用。详情: {gpu_error}"
    )))
}

fn docker_output_detail(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !stderr.is_empty() {
        return stderr;
    }
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn stop_container(name: &str) -> Result<(), SenseVoiceError> {
    let mut command = docker_command();
    command.arg("stop").arg(name);
    hide_window(&mut command);
    let output = command
        .output()
        .map_err(|err| SenseVoiceError::Process(err.to_string()))?;
    if output.status.success() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&output.stderr);
    if detail.contains("No such container") {
        return Ok(());
    }
    Err(SenseVoiceError::Process(format!(
        "停止容器失败: {}",
        detail.trim()
    )))
}

fn docker_container_state(name: &str) -> Result<RuntimeState, SenseVoiceError> {
    let mut command = docker_command();
    command
        .arg("inspect")
        .arg("-f")
        .arg("{{.State.Status}}")
        .arg(name);
    hide_window(&mut command);
    let output = command
        .output()
        .map_err(|err| SenseVoiceError::Process(err.to_string()))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        if detail.contains("No such object") || detail.contains("No such container") {
            return Ok(RuntimeState::Stopped);
        }
        return Err(SenseVoiceError::Process(format!(
            "读取容器状态失败: {}",
            detail.trim()
        )));
    }
    let status = String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_ascii_lowercase();
    if status == "running" {
        return Ok(RuntimeState::Running);
    }
    if status == "paused" {
        return Ok(RuntimeState::Paused);
    }
    if status == "exited" || status == "created" || status == "dead" {
        return Ok(RuntimeState::Exited);
    }
    Ok(RuntimeState::Stopped)
}

#[allow(dead_code)]
fn detect_any_runtime_state() -> RuntimeState {
    let container_name = docker_container_name();
    docker_container_state(container_name).unwrap_or(RuntimeState::Stopped)
}

/// 检测容器状态并返回容器名
#[allow(dead_code)]
fn detect_any_runtime_state_with_name() -> (RuntimeState, Option<String>) {
    let container_name = docker_container_name();
    let state = docker_container_state(container_name).unwrap_or(RuntimeState::Stopped);
    if state == RuntimeState::Running || state == RuntimeState::Paused {
        (state, Some(container_name.to_string()))
    } else {
        (state, None)
    }
}

fn pause_container(name: &str) -> Result<(), SenseVoiceError> {
    let mut command = docker_command();
    command.arg("pause").arg(name);
    hide_window(&mut command);
    let output = command
        .output()
        .map_err(|err| SenseVoiceError::Process(err.to_string()))?;
    if output.status.success() {
        return Ok(());
    }
    let detail = docker_output_detail(&output);
    if detail.contains("already paused") || detail.contains("No such container") {
        return Ok(());
    }
    Err(SenseVoiceError::Process(format!(
        "暂停容器失败: {}",
        detail.trim()
    )))
}

fn unpause_container(name: &str) -> Result<(), SenseVoiceError> {
    let mut command = docker_command();
    command.arg("unpause").arg(name);
    hide_window(&mut command);
    let output = command
        .output()
        .map_err(|err| SenseVoiceError::Process(err.to_string()))?;
    if output.status.success() {
        return Ok(());
    }
    let detail = docker_output_detail(&output);
    if detail.contains("is not paused") {
        return Ok(());
    }
    Err(SenseVoiceError::Process(format!(
        "恢复容器失败: {}",
        detail.trim()
    )))
}

fn pause_runtime_container_if_needed(name: &str) -> Result<bool, SenseVoiceError> {
    match docker_container_state(name)? {
        RuntimeState::Running => {
            pause_container(name)?;
            Ok(true)
        }
        RuntimeState::Paused => Ok(true),
        RuntimeState::Stopped | RuntimeState::Exited => Ok(false),
    }
}

fn unpause_runtime_container_if_needed(name: &str) -> Result<bool, SenseVoiceError> {
    match docker_container_state(name)? {
        RuntimeState::Paused => {
            unpause_container(name)?;
            Ok(true)
        }
        RuntimeState::Running => Ok(true),
        RuntimeState::Stopped | RuntimeState::Exited => Ok(false),
    }
}

#[allow(dead_code)]
fn pause_all_runtime_containers() -> bool {
    let container_name = docker_container_name();
    pause_runtime_container_if_needed(container_name).unwrap_or(false)
}

/// 停止运行时容器但保留（不删除），下次可通过 docker start 恢复
#[allow(dead_code)]
fn stop_all_runtime_containers_keep() {
    let container_name = docker_container_name();
    let _ = stop_container(container_name);
}

/// 清理旧版多容器遗留（一次性操作）
fn cleanup_legacy_containers(app: &AppHandle) {
    let app_data_dir = match app.path().app_data_dir() {
        Ok(dir) => dir,
        Err(_) => return,
    };
    let flag_file = app_data_dir.join(".legacy_containers_cleaned");
    if flag_file.exists() {
        return;
    }
    for name in legacy_container_names() {
        let _ = remove_container_if_exists(name);
    }
    let _ = fs::create_dir_all(&app_data_dir);
    let _ = fs::write(&flag_file, "done");
}

fn attach_runtime_logs(
    child: &mut Child,
    app: AppHandle,
    log_path: &Path,
    runtime_tail: Arc<Mutex<VecDeque<String>>>,
    startup_completed: Arc<AtomicBool>,
    stdout_file: fs::File,
    stderr_file: fs::File,
) -> Result<(), SenseVoiceError> {
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| SenseVoiceError::Process("SenseVoice 服务无法读取 stdout".to_string()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| SenseVoiceError::Process("SenseVoice 服务无法读取 stderr".to_string()))?;

    spawn_runtime_log_reader(
        stdout,
        "stdout",
        app.clone(),
        runtime_tail.clone(),
        startup_completed.clone(),
        stdout_file,
        log_path.to_path_buf(),
    );
    spawn_runtime_log_reader(
        stderr,
        "stderr",
        app,
        runtime_tail,
        startup_completed,
        stderr_file,
        log_path.to_path_buf(),
    );
    Ok(())
}

fn spawn_runtime_log_reader<R>(
    reader: R,
    stream: &str,
    app: AppHandle,
    runtime_tail: Arc<Mutex<VecDeque<String>>>,
    startup_completed: Arc<AtomicBool>,
    mut output_file: fs::File,
    log_path: PathBuf,
) where
    R: std::io::Read + Send + 'static,
{
    let stream_name = stream.to_string();
    thread::spawn(move || {
        // 使用 mpsc channel 将读取线程与 emit 节流逻辑解耦
        let (tx, rx) = mpsc::channel::<String>();
        let stream_for_reader = stream_name.clone();

        // 子线程：逐行读取，写文件 + 更新 runtime_tail + 发送到 channel
        let reader_handle = {
            let runtime_tail = Arc::clone(&runtime_tail);
            thread::spawn(move || {
                let buf = BufReader::new(reader);
                for line in buf.lines() {
                    let Ok(raw) = line else {
                        break;
                    };
                    let normalized = normalize_log_line(&raw);
                    if normalized.is_empty() {
                        continue;
                    }
                    if is_startup_complete_line(&normalized) {
                        startup_completed.store(true, Ordering::Relaxed);
                    }
                    let _ = writeln!(output_file, "{normalized}");
                    let _ = output_file.flush();
                    push_runtime_tail(
                        &runtime_tail,
                        format!("[{stream_for_reader}] {normalized}"),
                        200,
                    );
                    // channel 满了就丢弃（避免积压），不影响文件和 tail
                    let _ = tx.send(normalized);
                }
                let _ = output_file.flush();
            })
        };

        // 主线程：按 LOG_EMIT_INTERVAL_MILLIS 节流批量 emit
        let emit_interval = Duration::from_millis(LOG_EMIT_INTERVAL_MILLIS);
        let mut last_emit = Instant::now();
        let mut pending: Vec<String> = Vec::new();

        loop {
            // 收集当前积压的所有日志行
            loop {
                match rx.try_recv() {
                    Ok(line) => pending.push(line),
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        // reader 线程结束，flush 剩余数据后退出
                        flush_pending_logs(&app, &stream_name, &mut pending);
                        let _ = reader_handle.join();
                        if let Ok(mut fallback) =
                            OpenOptions::new().create(true).append(true).open(&log_path)
                        {
                            let _ = writeln!(fallback, "[{stream_name}] log stream ended");
                        }
                        return;
                    }
                }
            }

            if !pending.is_empty() && last_emit.elapsed() >= emit_interval {
                flush_pending_logs(&app, &stream_name, &mut pending);
                last_emit = Instant::now();
            }

            thread::sleep(Duration::from_millis(20));
        }
    });
}

/// 将积累的日志行批量以一次 app.emit() 发出（多行合并为换行分隔的字符串）
fn flush_pending_logs(app: &AppHandle, stream: &str, pending: &mut Vec<String>) {
    if pending.is_empty() {
        return;
    }
    let combined = pending.join("\n");
    pending.clear();
    let payload = SenseVoiceRuntimeLog {
        stream: stream.to_string(),
        line: combined,
        ts: current_timestamp_ms(),
    };
    let _ = app.emit("sensevoice-runtime-log", payload);
}

fn push_runtime_tail(tail: &Arc<Mutex<VecDeque<String>>>, value: String, max_lines: usize) {
    if let Ok(mut guard) = tail.lock() {
        if guard.len() >= max_lines {
            guard.pop_front();
        }
        guard.push_back(value);
    }
}

fn collect_runtime_tail(
    tail: &Arc<Mutex<VecDeque<String>>>,
    max_lines: usize,
    log_path: &Path,
) -> String {
    if let Ok(guard) = tail.lock() {
        if !guard.is_empty() {
            let size = guard.len();
            let start = size.saturating_sub(max_lines);
            return guard
                .iter()
                .skip(start)
                .cloned()
                .collect::<Vec<_>>()
                .join(" || ");
        }
    }
    read_log_tail(log_path, max_lines)
}

fn collect_runtime_tail_with_retry(
    tail: &Arc<Mutex<VecDeque<String>>>,
    max_lines: usize,
    log_path: &Path,
) -> String {
    let mut last = "（无日志）".to_string();
    for _ in 0..5 {
        let current = collect_runtime_tail(tail, max_lines, log_path);
        if current != "（无日志）" {
            return current;
        }
        last = current;
        thread::sleep(Duration::from_millis(120));
    }
    last
}

fn current_timestamp_ms() -> i64 {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    duration.as_millis() as i64
}

// ---------------------------------------------------------------------------
// vLLM 容器内模型调度：配置文件读写
// ---------------------------------------------------------------------------

/// 获取 vLLM 配置目录路径
fn vllm_config_dir(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join(VLLM_CONFIG_DIR_NAME)
}

/// 写入 vLLM 容器启动配置（entrypoint.sh + model.conf）
fn write_vllm_config(
    config_dir: &Path,
    model_key: &str,
    model_id: &str,
    port: u16,
    gpu_mem: f32,
    extra_args: &str,
) -> Result<(), SenseVoiceError> {
    fs::create_dir_all(config_dir).map_err(|err| SenseVoiceError::Io(err.to_string()))?;
    // entrypoint.sh
    fs::write(config_dir.join("entrypoint.sh"), VLLM_ENTRYPOINT_SH)
        .map_err(|err| SenseVoiceError::Io(err.to_string()))?;
    // model.conf（bash source 格式）
    let conf = format!(
        "MODEL_KEY={model_key}\nMODEL_ID={model_id}\nVLLM_PORT={port}\nVLLM_GPU_MEM={gpu_mem}\nVLLM_EXTRA_ARGS={extra_args}\n"
    );
    fs::write(config_dir.join("model.conf"), conf)
        .map_err(|err| SenseVoiceError::Io(err.to_string()))?;
    Ok(())
}

/// 从宿主机配置文件读取 vLLM 当前配置的模型信息
fn read_vllm_config_model(config_dir: &Path) -> Option<(String, String)> {
    let content = fs::read_to_string(config_dir.join("model.conf")).ok()?;
    let mut model_key = None;
    let mut model_id = None;
    for line in content.lines() {
        if let Some(val) = line.strip_prefix("MODEL_KEY=") {
            model_key = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("MODEL_ID=") {
            model_id = Some(val.trim().to_string());
        }
    }
    Some((model_key?, model_id?))
}

/// 构建 vLLM 模型的 extra_args 字符串
fn vllm_extra_args(local_model: &str) -> String {
    if local_model == LOCAL_MODEL_VOXTRAL {
        format!("--attention-backend {VOXTRAL_ATTENTION_BACKEND}")
    } else {
        "--max-model-len 12288".to_string()
    }
}

fn is_startup_complete_line(line: &str) -> bool {
    let lowered = line.to_lowercase();
    lowered.contains("application startup complete")
        || lowered.contains("started server process")
        || lowered.contains("uvicorn running on")
}

fn read_log_tail(path: &Path, max_lines: usize) -> String {
    let text = fs::read_to_string(path).unwrap_or_default();
    if text.trim().is_empty() {
        return "（无日志）".to_string();
    }

    let mut tail = VecDeque::with_capacity(max_lines);
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if tail.len() >= max_lines {
            tail.pop_front();
        }
        tail.push_back(trimmed.to_string());
    }

    if tail.is_empty() {
        "（无日志）".to_string()
    } else {
        tail.into_iter().collect::<Vec<_>>().join(" || ")
    }
}

/// 从 service_url 中提取端口号，供 docker exec 健康检查使用
fn extract_port_from_url(service_url: &str) -> Option<u16> {
    Url::parse(service_url)
        .ok()
        .and_then(|u| u.port_or_known_default())
}

/// 通过 docker exec 在容器内部执行 wget 健康检查，绕过 Docker Desktop/WSL2 端口映射延迟。
/// 容器内不一定有 curl，优先用 wget（BusyBox/Alpine 均内置），失败后尝试 python3。
fn docker_exec_health_check(container: &str, port: u16) -> bool {
    let mut health_urls = vec![format!("http://127.0.0.1:{port}/health")];
    if port != VLLM_INTERNAL_PORT {
        health_urls.push(format!("http://127.0.0.1:{VLLM_INTERNAL_PORT}/health"));
    }

    for health_url in health_urls {
        // 尝试 wget
        let wget_ok = {
            let mut cmd = docker_command();
            cmd.arg("exec")
                .arg(container)
                .arg("wget")
                .arg("-qO-")
                .arg("--timeout=2")
                .arg(&health_url);
            hide_window(&mut cmd);
            cmd.output()
                .map(|o| {
                    if o.status.success() {
                        let body = String::from_utf8_lossy(&o.stdout);
                        return body.contains("{") || body.len() > 0;
                    }
                    false
                })
                .unwrap_or(false)
        };
        if wget_ok {
            return true;
        }

        // 备选：python3 urllib
        let mut cmd = docker_command();
        cmd.arg("exec")
            .arg(container)
            .arg("python3")
            .arg("-c")
            .arg(format!(
                "import urllib.request; r=urllib.request.urlopen('{}',timeout=2); print(r.read())",
                health_url
            ));
        hide_window(&mut cmd);
        if cmd.output().map(|o| o.status.success()).unwrap_or(false) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{parse_health_ready_field, parse_vllm_models_response_ready};

    #[test]
    fn parse_health_ready_field_returns_true() {
        assert_eq!(
            parse_health_ready_field(r#"{"status":"ok","ready":true}"#),
            Some(true)
        );
    }

    #[test]
    fn parse_health_ready_field_returns_false() {
        assert_eq!(
            parse_health_ready_field(r#"{"status":"ok","ready":false}"#),
            Some(false)
        );
    }

    #[test]
    fn parse_health_ready_field_returns_none_when_missing() {
        assert_eq!(
            parse_health_ready_field(r#"{"status":"ok","loading":true}"#),
            None
        );
    }

    #[test]
    fn parse_vllm_models_response_ready_returns_true_for_non_empty_data() {
        assert!(parse_vllm_models_response_ready(
            r#"{"object":"list","data":[{"id":"Qwen/Qwen3-ASR-1.7B"}]}"#
        ));
    }

    #[test]
    fn parse_vllm_models_response_ready_returns_false_for_empty_data() {
        assert!(!parse_vllm_models_response_ready(
            r#"{"object":"list","data":[]}"#
        ));
    }
}
