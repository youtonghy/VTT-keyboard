use super::SenseVoiceError;
use crate::settings::SettingsStore;
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use url::Url;

const PREPARE_SCRIPT: &str = include_str!("scripts/prepare.py");
const SERVER_SCRIPT: &str = include_str!("scripts/server.py");
const REQUIREMENTS_TXT: &str = include_str!("scripts/requirements.txt");

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
}

pub struct SenseVoiceManager {
    child: Option<Child>,
}

impl SenseVoiceManager {
    pub fn new() -> Self {
        Self { child: None }
    }

    pub fn status(&mut self, store: &SettingsStore) -> Result<SenseVoiceStatus, SenseVoiceError> {
        let settings = store
            .load()
            .map_err(|err| SenseVoiceError::Settings(err.to_string()))?;
        Ok(SenseVoiceStatus {
            installed: settings.sensevoice.installed,
            enabled: settings.sensevoice.enabled,
            running: self.is_running(),
            service_url: settings.sensevoice.service_url,
            model_id: settings.sensevoice.model_id,
            device: settings.sensevoice.device,
            download_state: settings.sensevoice.download_state,
            last_error: settings.sensevoice.last_error,
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
            let settings = store
                .load()
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
            install_requirements(&venv_python, &paths.runtime_dir.join("requirements.txt"))?;

            self.update_state(store, "downloading", "", None, None)?;
            self.emit_progress(app, "download", "Downloading SenseVoice model", Some(60));
            download_model(
                &venv_python,
                &paths.runtime_dir.join("prepare.py"),
                &paths.models_dir,
                &paths.state_file,
                &settings.sensevoice.model_id,
                &settings.sensevoice.device,
            )?;

            self.update_state(store, "validating", "", None, None)?;
            self.emit_progress(app, "verify", "Starting SenseVoice service", Some(85));
            self.start_service(app, store)?;

            self.update_state(store, "ready", "", Some(true), Some(true))?;
            self.emit_progress(app, "done", "SenseVoice is ready", Some(100));
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

        let settings = store
            .load()
            .map_err(|err| SenseVoiceError::Settings(err.to_string()))?;
        if !settings.sensevoice.installed {
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

        let (host, port) = parse_host_and_port(&settings.sensevoice.service_url)?;
        let hub = read_selected_hub(&paths.state_file).unwrap_or_else(|| "hf".to_string());

        let mut command = Command::new(venv_python);
        command
            .arg(paths.runtime_dir.join("server.py"))
            .env("SENSEVOICE_MODEL_ID", settings.sensevoice.model_id.clone())
            .env(
                "SENSEVOICE_MODEL_DIR",
                paths.models_dir.to_string_lossy().to_string(),
            )
            .env("SENSEVOICE_DEVICE", settings.sensevoice.device.clone())
            .env("SENSEVOICE_HUB", hub)
            .env("SENSEVOICE_HOST", host)
            .env("SENSEVOICE_PORT", port.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let child = command
            .spawn()
            .map_err(|err| SenseVoiceError::Process(err.to_string()))?;
        self.child = Some(child);

        if let Err(err) = wait_health(&settings.sensevoice.service_url, Duration::from_secs(45)) {
            let _ = self.stop_service(app, store);
            return Err(err);
        }

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
        let mut settings = store
            .load()
            .map_err(|err| SenseVoiceError::Settings(err.to_string()))?;
        settings.sensevoice.download_state = download_state.to_string();
        settings.sensevoice.last_error = last_error.to_string();
        if let Some(next) = installed {
            settings.sensevoice.installed = next;
        }
        if let Some(next) = enabled {
            settings.sensevoice.enabled = next;
        }
        store
            .save(&settings)
            .map_err(|err| SenseVoiceError::Settings(err.to_string()))
    }

    fn emit_progress(&self, app: &AppHandle, stage: &str, message: &str, percent: Option<u8>) {
        let payload = SenseVoiceProgress {
            stage: stage.to_string(),
            message: message.to_string(),
            percent,
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

fn detect_system_python() -> Option<String> {
    for candidate in ["python3", "python"] {
        let status = Command::new(candidate)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if status.is_ok_and(|value| value.success()) {
            return Some(candidate.to_string());
        }
    }
    None
}

fn ensure_venv(system_python: &str, venv_dir: &Path) -> Result<(), SenseVoiceError> {
    let venv_python = venv_python_path(venv_dir);
    if venv_python.exists() {
        return Ok(());
    }
    let mut command = Command::new(system_python);
    command.arg("-m").arg("venv").arg(venv_dir);
    run_command(&mut command, "创建 Python venv")
}

fn install_requirements(venv_python: &Path, requirements: &Path) -> Result<(), SenseVoiceError> {
    let mut upgrade_pip = Command::new(venv_python);
    upgrade_pip
        .arg("-m")
        .arg("pip")
        .arg("install")
        .arg("--upgrade")
        .arg("pip");
    run_command(&mut upgrade_pip, "升级 pip")?;

    let mut install = Command::new(venv_python);
    install
        .arg("-m")
        .arg("pip")
        .arg("install")
        .arg("-r")
        .arg(requirements);
    run_command(&mut install, "安装 SenseVoice 依赖")
}

fn download_model(
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
    run_command(&mut command, "下载 SenseVoice 模型")
}

fn venv_python_path(venv_dir: &Path) -> PathBuf {
    if cfg!(target_os = "windows") {
        venv_dir.join("Scripts").join("python.exe")
    } else {
        venv_dir.join("bin").join("python")
    }
}

fn wait_health(service_url: &str, timeout: Duration) -> Result<(), SenseVoiceError> {
    let url = format!("{}/health", service_url.trim_end_matches('/'));
    let client = reqwest::blocking::Client::new();
    let started = Instant::now();
    while started.elapsed() < timeout {
        let response = client.get(&url).send();
        if let Ok(value) = response {
            if value.status().is_success() {
                return Ok(());
            }
        }
        thread::sleep(Duration::from_millis(500));
    }
    Err(SenseVoiceError::Request(
        "SenseVoice 服务启动超时，请检查 Python 依赖或模型下载状态".to_string(),
    ))
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
