use super::SenseVoiceError;
use crate::sensevoice::worker::{WorkerEvent, WorkerJob};
use crate::settings::SettingsStore;
use crate::AppState;
use serde::Serialize;
use serde_json::Value;
use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Write};
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};
use url::Url;

const PREPARE_SCRIPT: &str = include_str!("scripts/prepare.py");
const SERVER_SCRIPT: &str = include_str!("scripts/server.py");
const REQUIREMENTS_TXT: &str = include_str!("scripts/requirements.txt");
const DOCKERFILE_TXT: &str = include_str!("scripts/Dockerfile");

const SERVICE_IMAGE_TAG: &str = "vtt-sensevoice:local";
const SERVICE_CONTAINER_NAME: &str = "vtt-sensevoice-service";

const SERVICE_START_TIMEOUT_SECS: u64 = 90;
const HEALTH_REQUEST_TIMEOUT_SECS: u64 = 2;
const HEALTH_MONITOR_WARN_SECS: u64 = 120;
const HEALTH_MONITOR_INTERVAL_MILLIS: u64 = 1000;
const DOCKER_BUILD_TIMEOUT_SECS: u64 = 40 * 60;
const IMAGE_STAMP_FILE: &str = "image.stamp";
const WORKER_ARG: &str = "--sensevoice-worker";
const WORKER_JOB_FILE_ARG: &str = "--job-file";
const START_CANCELLED_MARKER: &str = "__sensevoice_start_cancelled__";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SenseVoiceStatus {
    pub installed: bool,
    pub enabled: bool,
    pub running: bool,
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
    detail: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SenseVoiceRuntimeLog {
    stream: String,
    line: String,
    ts: i64,
}

pub struct SenseVoiceManager {
    container_name: Option<String>,
    log_child: Option<Child>,
    prepare_child: Option<Child>,
    start_in_progress: bool,
    start_cancel_flag: Arc<AtomicBool>,
}

impl SenseVoiceManager {
    pub fn new() -> Self {
        Self {
            container_name: None,
            log_child: None,
            prepare_child: None,
            start_in_progress: false,
            start_cancel_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn status(&mut self, store: &SettingsStore) -> Result<SenseVoiceStatus, SenseVoiceError> {
        self.reconcile_prepare_task();
        let sensevoice = store
            .load_sensevoice()
            .map_err(|err| SenseVoiceError::Settings(err.to_string()))?;
        let running = self.is_running() || self.start_in_progress;
        Ok(SenseVoiceStatus {
            installed: sensevoice.installed,
            enabled: sensevoice.enabled,
            running,
            service_url: sensevoice.service_url,
            model_id: sensevoice.model_id,
            device: sensevoice.device,
            download_state: sensevoice.download_state,
            last_error: sensevoice.last_error,
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
        let paths = ensure_paths(app)?;
        write_runtime_files(&paths)?;
        let job = WorkerJob {
            service_url: sensevoice.service_url,
            model_id: sensevoice.model_id,
            device: sensevoice.device,
            runtime_dir: paths.runtime_dir.to_string_lossy().to_string(),
            models_dir: paths.models_dir.to_string_lossy().to_string(),
            state_file: paths.state_file.to_string_lossy().to_string(),
            image_tag: SERVICE_IMAGE_TAG.to_string(),
            container_name: SERVICE_CONTAINER_NAME.to_string(),
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
        if self.is_running() {
            self.start_in_progress = false;
            self.start_cancel_flag.store(false, Ordering::Relaxed);
            return self.status(store);
        }
        if self.start_in_progress {
            return self.status(store);
        }

        let sensevoice = store
            .load_sensevoice()
            .map_err(|err| SenseVoiceError::Settings(err.to_string()))?;
        if !sensevoice.installed {
            return Err(SenseVoiceError::Config(
                "SenseVoice 尚未安装，请先完成下载".to_string(),
            ));
        }

        self.start_in_progress = true;
        self.start_cancel_flag.store(false, Ordering::Relaxed);
        self.update_state(store, "running", "", None, None)?;

        let app_handle = app.clone();
        let store_clone = store.clone();
        let cancel_flag = Arc::clone(&self.start_cancel_flag);
        thread::spawn(move || {
            run_startup_task(app_handle, store_clone, cancel_flag);
        });

        self.status(store)
    }

    pub fn stop_service(
        &mut self,
        _app: &AppHandle,
        store: &SettingsStore,
    ) -> Result<SenseVoiceStatus, SenseVoiceError> {
        self.stop_prepare_task();
        self.start_cancel_flag.store(true, Ordering::Relaxed);
        self.start_in_progress = false;
        self.stop_log_stream();
        let _ = stop_container(SERVICE_CONTAINER_NAME);
        let _ = remove_container_if_exists(SERVICE_CONTAINER_NAME);
        self.container_name = None;
        let _ = self.update_state(store, "idle", "", None, None);
        self.status(store)
    }

    fn is_running(&mut self) -> bool {
        self.reconcile_prepare_task();
        let running = docker_container_running(SERVICE_CONTAINER_NAME).unwrap_or(false);
        if running {
            self.container_name = Some(SERVICE_CONTAINER_NAME.to_string());
        } else {
            self.container_name = None;
            self.stop_log_stream();
        }
        running
    }

    fn is_prepare_running(&mut self) -> bool {
        self.reconcile_prepare_task();
        self.prepare_child.is_some()
    }

    fn start_log_stream(
        &mut self,
        app: AppHandle,
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
            .arg(SERVICE_CONTAINER_NAME)
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
        let content =
            serde_json::to_string_pretty(job).map_err(|err| SenseVoiceError::Process(err.to_string()))?;
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
        self.emit_progress_detail(app, stage, message, percent, None);
    }

    fn emit_progress_detail(
        &self,
        app: &AppHandle,
        stage: &str,
        message: &str,
        percent: Option<u8>,
        detail: Option<&str>,
    ) {
        let payload = SenseVoiceProgress {
            stage: stage.to_string(),
            message: message.to_string(),
            percent,
            detail: detail.map(str::to_string),
        };
        let _ = app.emit("sensevoice-progress", payload);
    }
}

impl Drop for SenseVoiceManager {
    fn drop(&mut self) {
        self.stop_prepare_task();
        self.stop_log_stream();
        let _ = stop_container(SERVICE_CONTAINER_NAME);
        let _ = remove_container_if_exists(SERVICE_CONTAINER_NAME);
    }
}

fn run_startup_task(app: AppHandle, store: SettingsStore, cancel_flag: Arc<AtomicBool>) {
    let mut runtime_tail: Option<Arc<Mutex<VecDeque<String>>>> = None;
    let mut log_path: Option<PathBuf> = None;

    let result = (|| -> Result<(String, PathBuf, Arc<Mutex<VecDeque<String>>>), SenseVoiceError> {
        check_start_cancelled(&cancel_flag)?;
        let sensevoice = store
            .load_sensevoice()
            .map_err(|err| SenseVoiceError::Settings(err.to_string()))?;
        if !sensevoice.installed {
            return Err(SenseVoiceError::Config(
                "SenseVoice 尚未安装，请先完成下载".to_string(),
            ));
        }

        let paths = ensure_paths(&app)?;
        write_runtime_files(&paths)?;
        check_start_cancelled(&cancel_flag)?;
        ensure_docker_available()?;
        check_start_cancelled(&cancel_flag)?;
        ensure_runtime_image(&app, &paths.runtime_dir)?;
        check_start_cancelled(&cancel_flag)?;

        let (host, port) = parse_host_and_port(&sensevoice.service_url)?;
        let publish_host = normalize_publish_host(&host)?;
        let hub = read_selected_hub(&paths.state_file).unwrap_or_else(|| "hf".to_string());
        let current_log_path = paths.runtime_dir.join("server.log");
        log_path = Some(current_log_path.clone());

        let _ = remove_container_if_exists(SERVICE_CONTAINER_NAME);
        run_service_container(
            &publish_host,
            port,
            &paths.models_dir,
            &sensevoice.model_id,
            &sensevoice.device,
            &hub,
        )?;
        check_start_cancelled(&cancel_flag)?;

        let current_runtime_tail = Arc::new(Mutex::new(VecDeque::with_capacity(200)));
        runtime_tail = Some(Arc::clone(&current_runtime_tail));
        let startup_completed = Arc::new(AtomicBool::new(false));
        {
            let state = app.state::<AppState>();
            let mut manager = state
                .sensevoice_manager
                .lock()
                .map_err(|_| SenseVoiceError::Process("SenseVoice 状态锁获取失败".to_string()))?;
            manager.start_log_stream(
                app.clone(),
                &current_log_path,
                Arc::clone(&current_runtime_tail),
                Arc::clone(&startup_completed),
            )?;
            manager.container_name = Some(SERVICE_CONTAINER_NAME.to_string());
        }

        wait_service_reachable(
            &sensevoice.service_url,
            Duration::from_secs(SERVICE_START_TIMEOUT_SECS),
            &current_log_path,
            &current_runtime_tail,
            &cancel_flag,
        )?;
        update_state_in_store(&store, "running", "", None, None)?;
        emit_progress_payload(
            &app,
            "verify",
            "SenseVoice service started, model warming up",
            Some(85),
            Some("Service is reachable; model warmup is still running".to_string()),
        );

        Ok((sensevoice.service_url, current_log_path, current_runtime_tail))
    })();

    match result {
        Ok((service_url, monitor_log_path, monitor_runtime_tail)) => {
            finish_start_task(&app, &cancel_flag);
            spawn_health_monitor(
                app,
                store,
                service_url,
                monitor_log_path,
                monitor_runtime_tail,
                cancel_flag,
            );
        }
        Err(err) => {
            let cancelled = is_start_cancelled_error(&err) || cancel_flag.load(Ordering::Relaxed);
            handle_startup_failure(
                &app,
                &store,
                err,
                cancelled,
                runtime_tail.as_ref(),
                log_path.as_deref(),
            );
            finish_start_task(&app, &cancel_flag);
        }
    }
}

fn spawn_health_monitor(
    app: AppHandle,
    store: SettingsStore,
    service_url: String,
    log_path: PathBuf,
    runtime_tail: Arc<Mutex<VecDeque<String>>>,
    cancel_flag: Arc<AtomicBool>,
) {
    thread::spawn(move || {
        let client = health_client();
        let url = format!("{}/health", service_url.trim_end_matches('/'));
        let started = Instant::now();
        let mut warned = false;
        let mut last_warmup_emit = Instant::now()
            .checked_sub(Duration::from_secs(5))
            .unwrap_or_else(Instant::now);

        loop {
            if cancel_flag.load(Ordering::Relaxed) {
                return;
            }

            match docker_container_running(SERVICE_CONTAINER_NAME) {
                Ok(true) => {}
                Ok(false) => {
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

            if let Ok(response) = client.get(&url).send() {
                let status = response.status();
                let body = response.text().unwrap_or_default();
                if status.is_success() {
                    let ready = serde_json::from_str::<Value>(&body)
                        .ok()
                        .and_then(|json| json.get("ready").and_then(|v| v.as_bool()))
                        .unwrap_or(false);
                    if ready {
                        let _ = update_state_in_store(&store, "ready", "", None, None);
                        emit_progress_payload(
                            &app,
                            "done",
                            "SenseVoice service ready",
                            Some(100),
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
                );
                warned = true;
            }

            thread::sleep(Duration::from_millis(HEALTH_MONITOR_INTERVAL_MILLIS));
        }
    });
}

fn wait_service_reachable(
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
        match docker_container_running(SERVICE_CONTAINER_NAME) {
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

        if let Ok(response) = client.get(&url).send() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            let body_is_json = serde_json::from_str::<Value>(&body).is_ok();
            if status.is_success() || body_is_json {
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
    );
    cleanup_runtime_state(app);
    let _ = stop_container(SERVICE_CONTAINER_NAME);
    let _ = remove_container_if_exists(SERVICE_CONTAINER_NAME);
}

fn handle_startup_failure(
    app: &AppHandle,
    store: &SettingsStore,
    err: SenseVoiceError,
    cancelled: bool,
    runtime_tail: Option<&Arc<Mutex<VecDeque<String>>>>,
    log_path: Option<&Path>,
) {
    if cancelled {
        let _ = update_state_in_store(store, "idle", "", None, None);
    } else {
        let mut message = err.to_string();
        if let (Some(tail_store), Some(path)) = (runtime_tail, log_path) {
            let tail = collect_runtime_tail_with_retry(tail_store, 30, path);
            if tail != "（无日志）" {
                message = format!("{message}。最近日志: {tail}");
            }
        }
        let _ = update_state_in_store(store, "error", &message, None, None);
        emit_progress_payload(
            app,
            "error",
            "SenseVoice startup failed",
            None,
            Some(message),
        );
    }

    cleanup_runtime_state(app);
    let _ = stop_container(SERVICE_CONTAINER_NAME);
    let _ = remove_container_if_exists(SERVICE_CONTAINER_NAME);
}

fn cleanup_runtime_state(app: &AppHandle) {
    if let Ok(mut manager) = app.state::<AppState>().sensevoice_manager.lock() {
        manager.stop_log_stream();
        manager.container_name = None;
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

fn handle_worker_event(app: &AppHandle, store: &SettingsStore, event: WorkerEvent) {
    match event {
        WorkerEvent::Progress {
            stage,
            message,
            percent,
            detail,
        } => {
            emit_progress_payload(app, &stage, &message, percent, detail);
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
            emit_progress_payload(app, "done", "SenseVoice service started", Some(100), Some(message));
        }
        WorkerEvent::Error { message } => {
            let _ = update_state_in_store(store, "error", &message, None, None);
            emit_progress_payload(app, "error", &message, None, None);
        }
    }
}

fn emit_progress_payload(
    app: &AppHandle,
    stage: &str,
    message: &str,
    percent: Option<u8>,
    detail: Option<String>,
) {
    let payload = SenseVoiceProgress {
        stage: stage.to_string(),
        message: message.to_string(),
        percent,
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
    version.arg("version").arg("--format").arg("{{.Client.Version}}");
    hide_window(&mut version);
    let output = version
        .output()
        .map_err(|err| SenseVoiceError::Config(format!("未检测到 Docker，请先安装 Docker: {err}")))?;
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
    let has_image = docker_image_exists(SERVICE_IMAGE_TAG);

    if has_image && previous_stamp.trim() == expected_stamp {
        return Ok(());
    }

    let payload = SenseVoiceProgress {
        stage: "install".to_string(),
        message: "Building Docker image".to_string(),
        percent: Some(35),
        detail: Some(format!("Building image {SERVICE_IMAGE_TAG}")),
    };
    let _ = app.emit("sensevoice-progress", payload);

    let mut build = docker_command();
    build
        .arg("build")
        .arg("-t")
        .arg(SERVICE_IMAGE_TAG)
        .arg(runtime_dir);
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
                    detail: Some(detail),
                };
                let _ = app.emit("sensevoice-progress", payload);
            }
        },
    )?;

    fs::write(stamp_path, expected_stamp).map_err(|err| SenseVoiceError::Io(err.to_string()))?;
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

fn docker_image_exists(image: &str) -> bool {
    let mut inspect = docker_command();
    inspect.arg("image").arg("inspect").arg(image);
    hide_window(&mut inspect);
    inspect.status().is_ok_and(|status| status.success())
}

fn run_service_container(
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
        .arg(SERVICE_CONTAINER_NAME)
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
        .arg(SERVICE_IMAGE_TAG);

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

fn remove_container_if_exists(name: &str) -> Result<(), SenseVoiceError> {
    let mut command = docker_command();
    command.arg("rm").arg("-f").arg(name);
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
        "移除容器失败: {}",
        detail.trim()
    )))
}

fn docker_container_running(name: &str) -> Result<bool, SenseVoiceError> {
    let mut command = docker_command();
    command
        .arg("inspect")
        .arg("-f")
        .arg("{{.State.Running}}")
        .arg(name);
    hide_window(&mut command);
    let output = command
        .output()
        .map_err(|err| SenseVoiceError::Process(err.to_string()))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        if detail.contains("No such object") || detail.contains("No such container") {
            return Ok(false);
        }
        return Err(SenseVoiceError::Process(format!(
            "读取容器状态失败: {}",
            detail.trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim() == "true")
}

fn bind_mount(source: &Path, target: &str) -> String {
    format!(
        "type=bind,source={},target={target}",
        source.to_string_lossy()
    )
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
        let reader = BufReader::new(reader);
        for line in reader.lines() {
            let Ok(raw) = line else {
                continue;
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

            push_runtime_tail(&runtime_tail, format!("[{stream_name}] {normalized}"), 200);

            let payload = SenseVoiceRuntimeLog {
                stream: stream_name.clone(),
                line: normalized,
                ts: current_timestamp_ms(),
            };
            let _ = app.emit("sensevoice-runtime-log", payload);
        }

        let _ = output_file.flush();
        if let Ok(mut fallback) = OpenOptions::new().create(true).append(true).open(log_path) {
            let _ = writeln!(fallback, "[{stream_name}] log stream ended");
        }
    });
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

fn run_command_streaming<F>(
    command: &mut Command,
    step: &str,
    timeout: Duration,
    mut on_line: F,
) -> Result<(), SenseVoiceError>
where
    F: FnMut(&str),
{
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    hide_window(command);

    let mut child = command
        .spawn()
        .map_err(|err| SenseVoiceError::Process(err.to_string()))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| SenseVoiceError::Process(format!("{step}失败: 无法读取 stdout")))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| SenseVoiceError::Process(format!("{step}失败: 无法读取 stderr")))?;

    let (tx, rx) = mpsc::channel::<String>();
    let tx_out = tx.clone();
    let stdout_handle = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(value) = line {
                let _ = tx_out.send(value);
            }
        }
    });
    let stderr_handle = thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            if let Ok(value) = line {
                let _ = tx.send(value);
            }
        }
    });

    let started = Instant::now();
    let mut tail = VecDeque::with_capacity(30);
    let exit_status = loop {
        while let Ok(line) = rx.try_recv() {
            if !line.trim().is_empty() {
                if tail.len() >= 30 {
                    tail.pop_front();
                }
                tail.push_back(line.clone());
                on_line(&line);
            }
        }

        if started.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_handle.join();
            let _ = stderr_handle.join();
            return Err(SenseVoiceError::Process(format!(
                "{step}超时（{} 秒），请检查网络或重试。最近日志: {}",
                timeout.as_secs(),
                tail.into_iter().collect::<Vec<_>>().join(" || ")
            )));
        }

        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => thread::sleep(Duration::from_millis(200)),
            Err(err) => {
                let _ = stdout_handle.join();
                let _ = stderr_handle.join();
                return Err(SenseVoiceError::Process(format!("{step}失败: {err}")));
            }
        }
    };

    while let Ok(line) = rx.try_recv() {
        if !line.trim().is_empty() {
            if tail.len() >= 30 {
                tail.pop_front();
            }
            tail.push_back(line.clone());
            on_line(&line);
        }
    }

    let _ = stdout_handle.join();
    let _ = stderr_handle.join();

    if exit_status.success() {
        return Ok(());
    }

    Err(SenseVoiceError::Process(format!(
        "{step}失败: {}",
        tail.into_iter().collect::<Vec<_>>().join(" || ")
    )))
}

fn normalize_log_line(line: &str) -> String {
    line.replace('\r', "").trim().to_string()
}

fn parse_host_and_port(service_url: &str) -> Result<(String, u16), SenseVoiceError> {
    let parsed = Url::parse(service_url).map_err(|err| SenseVoiceError::Url(err.to_string()))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| SenseVoiceError::Url("服务地址缺少主机名".to_string()))?
        .to_string();
    let port = parsed
        .port_or_known_default()
        .ok_or_else(|| SenseVoiceError::Url("服务地址缺少端口".to_string()))?;
    Ok((host, port))
}

fn normalize_publish_host(host: &str) -> Result<String, SenseVoiceError> {
    if host.eq_ignore_ascii_case("localhost") {
        return Ok("127.0.0.1".to_string());
    }
    if host.parse::<Ipv4Addr>().is_ok() {
        return Ok(host.to_string());
    }
    Err(SenseVoiceError::Config(
        "Docker 模式下服务地址主机仅支持 localhost 或 IPv4 地址".to_string(),
    ))
}

fn read_selected_hub(state_file: &Path) -> Option<String> {
    let data = fs::read_to_string(state_file).ok()?;
    let value: Value = serde_json::from_str(&data).ok()?;
    value
        .get("hub")
        .and_then(|item| item.as_str())
        .map(str::to_string)
}

fn docker_command() -> Command {
    Command::new("docker")
}

fn hide_window(_command: &mut Command) {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        _command.creation_flags(0x0800_0000);
    }
}
