use super::SenseVoiceError;
use crate::settings::SettingsStore;
use serde::Serialize;
use serde_json::Value;
use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};
use url::Url;

const PREPARE_SCRIPT: &str = include_str!("scripts/prepare.py");
const SERVER_SCRIPT: &str = include_str!("scripts/server.py");
const REQUIREMENTS_TXT: &str = include_str!("scripts/requirements.txt");
const PIP_INSTALL_TIMEOUT_SECS: u64 = 20 * 60;
const PIP_DEFAULT_TIMEOUT_SECS: u64 = 60;
const PIP_RETRIES: u32 = 3;
const PIP_MIRRORS: [&str; 2] = [
    "https://pypi.tuna.tsinghua.edu.cn/simple",
    "https://pypi.org/simple",
];
const TORCH_INDEXES: [&str; 3] = [
    "https://download.pytorch.org/whl/cpu",
    "https://pypi.tuna.tsinghua.edu.cn/simple",
    "https://pypi.org/simple",
];
const SERVICE_START_TIMEOUT_SECS: u64 = 90;

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
    child: Option<Child>,
}

impl SenseVoiceManager {
    pub fn new() -> Self {
        Self { child: None }
    }

    pub fn status(&mut self, store: &SettingsStore) -> Result<SenseVoiceStatus, SenseVoiceError> {
        let sensevoice = store
            .load_sensevoice()
            .map_err(|err| SenseVoiceError::Settings(err.to_string()))?;
        Ok(SenseVoiceStatus {
            installed: sensevoice.installed,
            enabled: sensevoice.enabled,
            running: self.is_running(),
            service_url: sensevoice.service_url,
            model_id: sensevoice.model_id,
            device: sensevoice.device,
            download_state: sensevoice.download_state,
            last_error: sensevoice.last_error,
        })
    }

    pub fn prepare(
        &mut self,
        app: &AppHandle,
        store: &SettingsStore,
    ) -> Result<SenseVoiceStatus, SenseVoiceError> {
        self.emit_progress(app, "prepare", "Preparing runtime", Some(5));
        self.update_state(store, "preparing", "", None, None)?;

        let result: Result<SenseVoiceStatus, SenseVoiceError> = (|| {
            let sensevoice = store
                .load_sensevoice()
                .map_err(|err| SenseVoiceError::Settings(err.to_string()))?;
            let paths = ensure_paths(app)?;
            write_runtime_files(&paths.runtime_dir)?;

            let python = detect_system_python().ok_or_else(|| {
                SenseVoiceError::Config("未检测到 Python，请先安装 Python 3.10+".to_string())
            })?;

            self.emit_progress(app, "prepare", "Creating Python venv", Some(15));
            ensure_venv(&python, &paths.venv_dir)?;

            let venv_python = venv_python_path(&paths.venv_dir);
            if !venv_python.exists() {
                return Err(SenseVoiceError::Config(
                    "Python venv 初始化失败，未找到可执行文件".to_string(),
                ));
            }

            self.emit_progress(app, "install", "Installing Python dependencies", Some(35));
            install_requirements(app, &venv_python, &paths.runtime_dir.join("requirements.txt"))?;

            self.update_state(store, "downloading", "", None, None)?;
            self.emit_progress(app, "download", "Downloading SenseVoice model", Some(60));
            download_model(
                app,
                &venv_python,
                &paths.runtime_dir.join("prepare.py"),
                &paths.models_dir,
                &paths.state_file,
                &sensevoice.model_id,
                &sensevoice.device,
            )?;

            self.update_state(store, "validating", "", Some(true), None)?;
            self.emit_progress(app, "verify", "Starting SenseVoice service", Some(85));
            self.start_service(app, store)?;

            self.update_state(store, "ready", "", Some(true), Some(true))?;
            self.emit_progress(app, "done", "SenseVoice service started", Some(100));
            self.status(store)
        })();

        if let Err(err) = &result {
            let _ = self.update_state(store, "error", &err.to_string(), None, None);
            self.emit_progress(app, "error", &err.to_string(), None);
        }

        result
    }

    pub fn start_service(
        &mut self,
        app: &AppHandle,
        store: &SettingsStore,
    ) -> Result<SenseVoiceStatus, SenseVoiceError> {
        if self.is_running() {
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

        let paths = ensure_paths(app)?;
        write_runtime_files(&paths.runtime_dir)?;
        let venv_python = venv_python_path(&paths.venv_dir);
        if !venv_python.exists() {
            return Err(SenseVoiceError::Config(
                "未找到 SenseVoice Python 环境，请先执行下载".to_string(),
            ));
        }

        let (host, port) = parse_host_and_port(&sensevoice.service_url)?;
        let hub = read_selected_hub(&paths.state_file).unwrap_or_else(|| "hf".to_string());
        let log_path = paths.runtime_dir.join("server.log");
        let mut log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|err| SenseVoiceError::Io(format!("打开 SenseVoice 日志失败: {err}")))?;
        let _ = writeln!(log_file, "\n=== sensevoice service start ===");
        let stdout_file = log_file
            .try_clone()
            .map_err(|err| SenseVoiceError::Io(format!("复制日志句柄失败: {err}")))?;

        let mut command = Command::new(venv_python);
        command
            .arg(paths.runtime_dir.join("server.py"))
            .env("SENSEVOICE_MODEL_ID", sensevoice.model_id.clone())
            .env(
                "SENSEVOICE_MODEL_DIR",
                paths.models_dir.to_string_lossy().to_string(),
            )
            .env("SENSEVOICE_DEVICE", sensevoice.device.clone())
            .env("SENSEVOICE_HUB", hub)
            .env("SENSEVOICE_HOST", host)
            .env("SENSEVOICE_PORT", port.to_string())
            .env("PYTHONUNBUFFERED", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = command
            .spawn()
            .map_err(|err| SenseVoiceError::Process(err.to_string()))?;
        let runtime_tail = Arc::new(Mutex::new(VecDeque::with_capacity(200)));
        attach_runtime_logs(
            &mut child,
            app.clone(),
            &log_path,
            Arc::clone(&runtime_tail),
            stdout_file,
            log_file,
        )?;

        if let Err(err) = wait_health(
            &sensevoice.service_url,
            Duration::from_secs(SERVICE_START_TIMEOUT_SECS),
            &mut child,
            &log_path,
            &runtime_tail,
        ) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(err);
        }

        self.child = Some(child);
        self.status(store)
    }

    pub fn stop_service(
        &mut self,
        _app: &AppHandle,
        store: &SettingsStore,
    ) -> Result<SenseVoiceStatus, SenseVoiceError> {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.status(store)
    }

    fn is_running(&mut self) -> bool {
        let mut clear = false;
        let running = if let Some(child) = self.child.as_mut() {
            match child.try_wait() {
                Ok(Some(_)) => {
                    clear = true;
                    false
                }
                Ok(None) => true,
                Err(_) => {
                    clear = true;
                    false
                }
            }
        } else {
            false
        };
        if clear {
            self.child = None;
        }
        running
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

struct SenseVoicePaths {
    runtime_dir: PathBuf,
    venv_dir: PathBuf,
    models_dir: PathBuf,
    state_file: PathBuf,
}

#[derive(Clone)]
struct PythonCommand {
    executable: String,
    prefix_args: Vec<String>,
}

fn ensure_paths(app: &AppHandle) -> Result<SenseVoicePaths, SenseVoiceError> {
    let root = app
        .path()
        .app_local_data_dir()
        .map_err(|err| SenseVoiceError::Io(err.to_string()))?
        .join("sensevoice");
    let runtime_dir = root.join("runtime");
    let venv_dir = root.join("venv");
    let models_dir = root.join("models");
    let state_file = root.join("state.json");
    fs::create_dir_all(&runtime_dir).map_err(|err| SenseVoiceError::Io(err.to_string()))?;
    fs::create_dir_all(&models_dir).map_err(|err| SenseVoiceError::Io(err.to_string()))?;
    Ok(SenseVoicePaths {
        runtime_dir,
        venv_dir,
        models_dir,
        state_file,
    })
}

fn write_runtime_files(runtime_dir: &Path) -> Result<(), SenseVoiceError> {
    fs::create_dir_all(runtime_dir).map_err(|err| SenseVoiceError::Io(err.to_string()))?;
    fs::write(runtime_dir.join("prepare.py"), PREPARE_SCRIPT)
        .map_err(|err| SenseVoiceError::Io(err.to_string()))?;
    fs::write(runtime_dir.join("server.py"), SERVER_SCRIPT)
        .map_err(|err| SenseVoiceError::Io(err.to_string()))?;
    fs::write(runtime_dir.join("requirements.txt"), REQUIREMENTS_TXT)
        .map_err(|err| SenseVoiceError::Io(err.to_string()))?;
    Ok(())
}

fn detect_system_python() -> Option<PythonCommand> {
    let mut candidates = Vec::new();
    if cfg!(target_os = "windows") {
        candidates.push(PythonCommand {
            executable: "py".to_string(),
            prefix_args: vec!["-3.11".to_string()],
        });
        candidates.push(PythonCommand {
            executable: "py".to_string(),
            prefix_args: vec!["-3.10".to_string()],
        });
        candidates.push(PythonCommand {
            executable: "py".to_string(),
            prefix_args: vec!["-3".to_string()],
        });
    }
    candidates.push(PythonCommand {
        executable: "python3".to_string(),
        prefix_args: Vec::new(),
    });
    candidates.push(PythonCommand {
        executable: "python".to_string(),
        prefix_args: Vec::new(),
    });

    for candidate in candidates {
        let mut command = Command::new(&candidate.executable);
        for arg in &candidate.prefix_args {
            command.arg(arg);
        }
        let status = command
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if status.is_ok_and(|value| value.success()) {
            return Some(candidate);
        }
    }
    None
}

fn ensure_venv(system_python: &PythonCommand, venv_dir: &Path) -> Result<(), SenseVoiceError> {
    let venv_python = venv_python_path(venv_dir);
    if venv_python.exists() {
        return Ok(());
    }
    let mut command = Command::new(&system_python.executable);
    for arg in &system_python.prefix_args {
        command.arg(arg);
    }
    command.arg("-m").arg("venv").arg(venv_dir);
    run_command(&mut command, "创建 Python venv")
}

fn install_requirements(
    app: &AppHandle,
    venv_python: &Path,
    requirements: &Path,
) -> Result<(), SenseVoiceError> {
    let mut upgrade_pip = Command::new(venv_python);
    upgrade_pip
        .arg("-m")
        .arg("pip")
        .arg("install")
        .arg("--upgrade")
        .arg("--progress-bar")
        .arg("off")
        .arg("--disable-pip-version-check")
        .arg("--default-timeout")
        .arg(PIP_DEFAULT_TIMEOUT_SECS.to_string())
        .arg("--retries")
        .arg(PIP_RETRIES.to_string())
        .arg("pip");
    run_command_streaming(
        &mut upgrade_pip,
        "升级 pip",
        Duration::from_secs(PIP_INSTALL_TIMEOUT_SECS),
        |line| {
            let detail = normalize_log_line(line);
            if !detail.is_empty() {
                let payload = SenseVoiceProgress {
                    stage: "install".to_string(),
                    message: "Installing Python dependencies".to_string(),
                    percent: Some(35),
                    detail: Some(detail),
                };
                let _ = app.emit("sensevoice-progress", payload);
            }
        },
    )?;

    let mut errors = Vec::new();
    for mirror in PIP_MIRRORS {
        let mirror_msg = format!("Using pip mirror: {mirror}");
        let payload = SenseVoiceProgress {
            stage: "install".to_string(),
            message: "Installing Python dependencies".to_string(),
            percent: Some(35),
            detail: Some(mirror_msg),
        };
        let _ = app.emit("sensevoice-progress", payload);

        let mut install = Command::new(venv_python);
        install
            .arg("-m")
            .arg("pip")
            .arg("install")
            .arg("--index-url")
            .arg(mirror)
            .arg("--progress-bar")
            .arg("off")
            .arg("--disable-pip-version-check")
            .arg("--default-timeout")
            .arg(PIP_DEFAULT_TIMEOUT_SECS.to_string())
            .arg("--retries")
            .arg(PIP_RETRIES.to_string())
            .arg("-r")
            .arg(requirements);

        match run_command_streaming(
            &mut install,
            "安装 SenseVoice 依赖",
            Duration::from_secs(PIP_INSTALL_TIMEOUT_SECS),
            |line| {
                let detail = normalize_log_line(line);
                if !detail.is_empty() {
                    let payload = SenseVoiceProgress {
                        stage: "install".to_string(),
                        message: "Installing Python dependencies".to_string(),
                        percent: Some(35),
                        detail: Some(detail),
                    };
                    let _ = app.emit("sensevoice-progress", payload);
                }
            },
        ) {
            Ok(_) => {
                match verify_runtime_imports(app, venv_python) {
                    Ok(_) => return Ok(()),
                    Err(err) => {
                        let detail = format!("Runtime verify failed: {err}");
                        let payload = SenseVoiceProgress {
                            stage: "install".to_string(),
                            message: "Installing Python dependencies".to_string(),
                            percent: Some(35),
                            detail: Some(detail),
                        };
                        let _ = app.emit("sensevoice-progress", payload);
                        if err.to_string().contains("No module named 'torch'") {
                            install_torch_packages(app, venv_python)?;
                            verify_runtime_imports(app, venv_python)?;
                            return Ok(());
                        }
                        errors.push(format!("{mirror}: {err}"));
                    }
                }
            }
            Err(err) => errors.push(format!("{mirror}: {err}")),
        }
    }

    Err(SenseVoiceError::Process(format!(
        "安装 SenseVoice 依赖失败（已尝试全部镜像）: {}",
        errors.join(" | ")
    )))
}

fn install_torch_packages(app: &AppHandle, venv_python: &Path) -> Result<(), SenseVoiceError> {
    let mut errors = Vec::new();
    for index in TORCH_INDEXES {
        let detail = format!("Retry install torch via index: {index}");
        let payload = SenseVoiceProgress {
            stage: "install".to_string(),
            message: "Installing Python dependencies".to_string(),
            percent: Some(35),
            detail: Some(detail),
        };
        let _ = app.emit("sensevoice-progress", payload);

        let mut install = Command::new(venv_python);
        install
            .arg("-m")
            .arg("pip")
            .arg("install")
            .arg("--index-url")
            .arg(index)
            .arg("--progress-bar")
            .arg("off")
            .arg("--disable-pip-version-check")
            .arg("--default-timeout")
            .arg(PIP_DEFAULT_TIMEOUT_SECS.to_string())
            .arg("--retries")
            .arg(PIP_RETRIES.to_string())
            .arg("torch")
            .arg("torchaudio");

        match run_command_streaming(
            &mut install,
            "补装 torch 依赖",
            Duration::from_secs(PIP_INSTALL_TIMEOUT_SECS),
            |line| {
                let detail = normalize_log_line(line);
                if !detail.is_empty() {
                    let payload = SenseVoiceProgress {
                        stage: "install".to_string(),
                        message: "Installing Python dependencies".to_string(),
                        percent: Some(35),
                        detail: Some(detail),
                    };
                    let _ = app.emit("sensevoice-progress", payload);
                }
            },
        ) {
            Ok(_) => return Ok(()),
            Err(err) => errors.push(format!("{index}: {err}")),
        }
    }

    Err(SenseVoiceError::Process(format!(
        "补装 torch 依赖失败（已尝试全部镜像）: {}",
        errors.join(" | ")
    )))
}

fn verify_runtime_imports(app: &AppHandle, venv_python: &Path) -> Result<(), SenseVoiceError> {
    let mut verify = Command::new(venv_python);
    verify.arg("-c").arg(
        "import sys; import torch; import funasr; print(f'python={sys.version.split()[0]} torch={torch.__version__}')",
    );

    run_command_streaming(
        &mut verify,
        "校验 SenseVoice 运行时依赖",
        Duration::from_secs(120),
        |line| {
            let detail = normalize_log_line(line);
            if !detail.is_empty() {
                let payload = SenseVoiceProgress {
                    stage: "install".to_string(),
                    message: "Installing Python dependencies".to_string(),
                    percent: Some(35),
                    detail: Some(detail),
                };
                let _ = app.emit("sensevoice-progress", payload);
            }
        },
    )
}

fn download_model(
    app: &AppHandle,
    venv_python: &Path,
    prepare_script: &Path,
    model_dir: &Path,
    state_file: &Path,
    model_id: &str,
    device: &str,
) -> Result<(), SenseVoiceError> {
    fs::create_dir_all(model_dir).map_err(|err| SenseVoiceError::Io(err.to_string()))?;

    let mut command = Command::new(venv_python);
    command
        .arg(prepare_script)
        .arg("--model-id")
        .arg(model_id)
        .arg("--model-dir")
        .arg(model_dir)
        .arg("--device")
        .arg(device)
        .arg("--hubs")
        .arg("hf,ms")
        .arg("--state-path")
        .arg(state_file);

    let first = run_command(&mut command, "下载 SenseVoice 模型");
    if let Err(err) = first {
        let err_text = err.to_string();
        if err_text.contains("No module named 'torch'") {
            let payload = SenseVoiceProgress {
                stage: "download".to_string(),
                message: "Downloading SenseVoice model".to_string(),
                percent: Some(60),
                detail: Some("Torch missing during model download, trying auto repair".to_string()),
            };
            let _ = app.emit("sensevoice-progress", payload);

            install_torch_packages(app, venv_python)?;

            let mut retry = Command::new(venv_python);
            retry
                .arg(prepare_script)
                .arg("--model-id")
                .arg(model_id)
                .arg("--model-dir")
                .arg(model_dir)
                .arg("--device")
                .arg(device)
                .arg("--hubs")
                .arg("hf,ms")
                .arg("--state-path")
                .arg(state_file);
            return run_command(&mut retry, "下载 SenseVoice 模型");
        }
        return Err(err);
    }

    Ok(())
}

fn venv_python_path(venv_dir: &Path) -> PathBuf {
    if cfg!(target_os = "windows") {
        venv_dir.join("Scripts").join("python.exe")
    } else {
        venv_dir.join("bin").join("python")
    }
}

fn wait_health(
    service_url: &str,
    timeout: Duration,
    child: &mut Child,
    log_path: &Path,
    runtime_tail: &Arc<Mutex<VecDeque<String>>>,
) -> Result<(), SenseVoiceError> {
    let url = format!("{}/health", service_url.trim_end_matches('/'));
    let client = reqwest::blocking::Client::new();
    let started = Instant::now();
    while started.elapsed() < timeout {
        match child.try_wait() {
            Ok(Some(status)) => {
                let tail = collect_runtime_tail(runtime_tail, 30, log_path);
                let exit = status
                    .code()
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "signal".to_string());
                return Err(SenseVoiceError::Request(format!(
                    "SenseVoice 服务进程已退出（code={exit}）。最近日志: {tail}"
                )));
            }
            Ok(None) => {}
            Err(err) => {
                return Err(SenseVoiceError::Request(format!(
                    "SenseVoice 服务状态检查失败: {err}"
                )));
            }
        }

        let response = client.get(&url).send();
        if let Ok(value) = response {
            if value.status().is_success() {
                return Ok(());
            }
        }
        thread::sleep(Duration::from_millis(500));
    }
    let tail = read_log_tail(log_path, 30);
    let tail = if tail == "（无日志）" {
        collect_runtime_tail(runtime_tail, 30, log_path)
    } else {
        tail
    };
    Err(SenseVoiceError::Request(
        format!(
            "SenseVoice 服务启动超时（{} 秒）。最近日志: {}",
            timeout.as_secs(),
            tail
        ),
    ))
}

fn attach_runtime_logs(
    child: &mut Child,
    app: AppHandle,
    log_path: &Path,
    runtime_tail: Arc<Mutex<VecDeque<String>>>,
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
        stdout_file,
        log_path.to_path_buf(),
    );
    spawn_runtime_log_reader(
        stderr,
        "stderr",
        app,
        runtime_tail,
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

            let _ = writeln!(output_file, "{normalized}");
            let _ = output_file.flush();

            push_runtime_tail(
                &runtime_tail,
                format!("[{stream_name}] {normalized}"),
                200,
            );

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

fn current_timestamp_ms() -> i64 {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    duration.as_millis() as i64
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

fn run_command(command: &mut Command, step: &str) -> Result<(), SenseVoiceError> {
    let output = command
        .output()
        .map_err(|err| SenseVoiceError::Process(err.to_string()))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let details = if !stderr.trim().is_empty() {
        stderr.trim().to_string()
    } else {
        stdout.trim().to_string()
    };
    Err(SenseVoiceError::Process(format!("{step}失败: {details}")))
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

fn read_selected_hub(state_file: &Path) -> Option<String> {
    let data = fs::read_to_string(state_file).ok()?;
    let value: Value = serde_json::from_str(&data).ok()?;
    value.get("hub").and_then(|item| item.as_str()).map(str::to_string)
}
