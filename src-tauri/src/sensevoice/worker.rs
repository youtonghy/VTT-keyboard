use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Write};
use std::net::Ipv4Addr;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use url::Url;

const SERVICE_START_TIMEOUT_SECS: u64 = 90;
const DOCKER_BUILD_TIMEOUT_SECS: u64 = 40 * 60;
const MODEL_DOWNLOAD_TIMEOUT_SECS: u64 = 60 * 60;
const IMAGE_STAMP_FILE: &str = "image.stamp";
const LOCAL_MODEL_SENSEVOICE: &str = "sensevoice";
const LOCAL_MODEL_VOXTRAL: &str = "voxtral";
const VOXTRAL_IMAGE_TAG: &str = "vllm/vllm-openai:latest";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerJob {
    #[serde(default = "default_local_model")]
    pub local_model: String,
    pub service_url: String,
    pub model_id: String,
    pub device: String,
    pub runtime_dir: String,
    pub models_dir: String,
    pub state_file: String,
    pub image_tag: String,
    pub container_name: String,
}

fn default_local_model() -> String {
    LOCAL_MODEL_SENSEVOICE.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum WorkerEvent {
    Progress {
        stage: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        percent: Option<u8>,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    RuntimeLog {
        stream: String,
        line: String,
        ts: i64,
    },
    State {
        download_state: String,
        last_error: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        installed: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        enabled: Option<bool>,
    },
    Done {
        message: String,
    },
    Error {
        message: String,
    },
}

pub fn run_worker(job_file: &str) -> i32 {
    let run_result = (|| -> Result<(), String> {
        let data = fs::read_to_string(job_file).map_err(|err| format!("读取任务文件失败: {err}"))?;
        let job: WorkerJob = serde_json::from_str(&data).map_err(|err| format!("任务参数解析失败: {err}"))?;
        run_prepare_job(&job)
    })();

    if let Err(err) = run_result {
        emit_event(&WorkerEvent::State {
            download_state: "error".to_string(),
            last_error: err.clone(),
            installed: None,
            enabled: None,
        });
        emit_event(&WorkerEvent::Error { message: err });
        return 1;
    }
    0
}

fn run_prepare_job(job: &WorkerJob) -> Result<(), String> {
    let local_model = normalize_local_model(&job.local_model);
    emit_progress("prepare", "Preparing runtime", Some(5), None);
    emit_state("preparing", "", None, None);

    ensure_docker_available()?;

    if local_model == LOCAL_MODEL_VOXTRAL {
        emit_progress("install", "Pulling Voxtral Docker image", Some(35), None);
        ensure_voxtral_image(|line| {
            emit_progress(
                "install",
                "Pulling Voxtral Docker image",
                Some(35),
                Some(line.to_string()),
            );
        })?;
        emit_state("ready", "", Some(true), Some(true));
        emit_progress("done", "Voxtral runtime prepared", Some(100), None);
        emit_event(&WorkerEvent::Done {
            message: "Voxtral prepare completed".to_string(),
        });
        return Ok(());
    }

    emit_progress("install", "Building Docker image", Some(35), None);
    ensure_runtime_image(job, |line| {
        emit_progress(
            "install",
            "Building Docker image",
            Some(35),
            Some(line.to_string()),
        );
    })?;

    emit_state("downloading", "", None, None);
    emit_progress("download", "Downloading SenseVoice model", Some(60), None);
    download_model(job, |line| {
        emit_progress(
            "download",
            "Downloading SenseVoice model",
            Some(60),
            Some(line.to_string()),
        );
    })?;

    emit_state("validating", "", Some(true), None);
    emit_progress("verify", "Starting SenseVoice service", Some(85), None);
    start_service(job)?;

    emit_state("ready", "", Some(true), Some(true));
    emit_progress("done", "SenseVoice service started", Some(100), None);
    emit_event(&WorkerEvent::Done {
        message: "SenseVoice prepare completed".to_string(),
    });
    Ok(())
}

fn ensure_docker_available() -> Result<(), String> {
    let mut version = docker_command();
    version.arg("version").arg("--format").arg("{{.Client.Version}}");
    hide_window(&mut version);
    let output = version
        .output()
        .map_err(|err| format!("未检测到 Docker，请先安装 Docker: {err}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!(
            "Docker 不可用，请先安装 Docker Desktop 并确保 docker 命令可执行: {detail}"
        ));
    }

    let mut info = docker_command();
    info.arg("info");
    hide_window(&mut info);
    let output = info
        .output()
        .map_err(|err| format!("无法连接 Docker daemon: {err}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!("Docker daemon 未运行，请先启动 Docker Desktop: {detail}"));
    }
    Ok(())
}

fn ensure_runtime_image<F>(job: &WorkerJob, mut on_line: F) -> Result<(), String>
where
    F: FnMut(&str),
{
    let runtime_dir = Path::new(&job.runtime_dir);
    let stamp_path = runtime_dir.join(IMAGE_STAMP_FILE);
    let expected_stamp = runtime_stamp(runtime_dir)?;
    let previous_stamp = fs::read_to_string(&stamp_path).unwrap_or_default();
    let has_image = docker_image_exists(&job.image_tag);
    if has_image && previous_stamp.trim() == expected_stamp {
        return Ok(());
    }

    let mut build = docker_command();
    build
        .arg("build")
        .arg("-t")
        .arg(&job.image_tag)
        .arg(runtime_dir);
    run_command_streaming(
        &mut build,
        "构建 SenseVoice Docker 镜像",
        Duration::from_secs(DOCKER_BUILD_TIMEOUT_SECS),
        |line| {
            let detail = normalize_log_line(line);
            if !detail.is_empty() {
                on_line(&detail);
            }
        },
    )?;

    fs::write(stamp_path, expected_stamp).map_err(|err| format!("写入镜像状态失败: {err}"))?;
    Ok(())
}

fn ensure_voxtral_image<F>(mut on_line: F) -> Result<(), String>
where
    F: FnMut(&str),
{
    if docker_image_exists(VOXTRAL_IMAGE_TAG) {
        return Ok(());
    }
    let mut pull = docker_command();
    pull.arg("pull").arg(VOXTRAL_IMAGE_TAG);
    run_command_streaming(
        &mut pull,
        "拉取 Voxtral Docker 镜像",
        Duration::from_secs(DOCKER_BUILD_TIMEOUT_SECS),
        |line| {
            let detail = normalize_log_line(line);
            if !detail.is_empty() {
                on_line(&detail);
            }
        },
    )
}

fn runtime_stamp(runtime_dir: &Path) -> Result<String, String> {
    let files = ["prepare.py", "server.py", "requirements.txt", "Dockerfile"];
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for file in files {
        let data =
            fs::read(runtime_dir.join(file)).map_err(|err| format!("读取运行时文件失败({file}): {err}"))?;
        data.hash(&mut hasher);
    }
    Ok(format!("{:x}", hasher.finish()))
}

fn normalize_local_model(value: &str) -> &str {
    if value.eq_ignore_ascii_case(LOCAL_MODEL_VOXTRAL) {
        LOCAL_MODEL_VOXTRAL
    } else {
        LOCAL_MODEL_SENSEVOICE
    }
}

fn docker_image_exists(image: &str) -> bool {
    let mut inspect = docker_command();
    inspect.arg("image").arg("inspect").arg(image);
    hide_window(&mut inspect);
    inspect.status().is_ok_and(|status| status.success())
}

fn download_model<F>(job: &WorkerJob, mut on_line: F) -> Result<(), String>
where
    F: FnMut(&str),
{
    let model_dir = Path::new(&job.models_dir);
    let state_file = Path::new(&job.state_file);
    fs::create_dir_all(model_dir).map_err(|err| format!("创建模型目录失败: {err}"))?;
    let state_dir = state_file
        .parent()
        .ok_or_else(|| "state.json 路径异常".to_string())?;
    fs::create_dir_all(state_dir).map_err(|err| format!("创建状态目录失败: {err}"))?;
    if !state_file.exists() {
        fs::write(state_file, "{}").map_err(|err| format!("初始化状态文件失败: {err}"))?;
    }

    let mut command = docker_command();
    command
        .arg("run")
        .arg("--rm")
        .arg("--mount")
        .arg(bind_mount(model_dir, "/models"))
        .arg("--mount")
        .arg(bind_mount(state_dir, "/state"))
        .arg(&job.image_tag)
        .arg("python")
        .arg("prepare.py")
        .arg("--model-id")
        .arg(&job.model_id)
        .arg("--model-dir")
        .arg("/models")
        .arg("--device")
        .arg(&job.device)
        .arg("--hubs")
        .arg("hf,ms")
        .arg("--state-path")
        .arg("/state/state.json");

    run_command_streaming(
        &mut command,
        "下载 SenseVoice 模型",
        Duration::from_secs(MODEL_DOWNLOAD_TIMEOUT_SECS),
        |line| {
            let detail = normalize_log_line(line);
            if !detail.is_empty() {
                on_line(&detail);
            }
        },
    )
}

fn start_service(job: &WorkerJob) -> Result<(), String> {
    let (host, port) = parse_host_and_port(&job.service_url)?;
    let publish_host = normalize_publish_host(&host)?;
    let hub = read_selected_hub(Path::new(&job.state_file)).unwrap_or_else(|| "hf".to_string());
    let model_dir = Path::new(&job.models_dir);

    let _ = remove_container_if_exists(&job.container_name);
    run_service_container(
        &job.container_name,
        &job.image_tag,
        &publish_host,
        port,
        model_dir,
        &job.model_id,
        &job.device,
        &hub,
    )?;

    wait_health(&job.container_name, &job.service_url, Duration::from_secs(SERVICE_START_TIMEOUT_SECS))
}

fn run_service_container(
    container_name: &str,
    image_tag: &str,
    publish_host: &str,
    port: u16,
    model_dir: &Path,
    model_id: &str,
    device: &str,
    hub: &str,
) -> Result<(), String> {
    fs::create_dir_all(model_dir).map_err(|err| format!("创建模型目录失败: {err}"))?;
    let mut command = docker_command();
    command
        .arg("run")
        .arg("-d")
        .arg("--name")
        .arg(container_name)
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
        .arg(image_tag);
    hide_window(&mut command);
    let output = command
        .output()
        .map_err(|err| format!("启动 SenseVoice 容器失败: {err}"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let detail = if !stderr.is_empty() { stderr } else { stdout };
    Err(format!("启动 SenseVoice 容器失败: {detail}"))
}

fn wait_health(container_name: &str, service_url: &str, timeout: Duration) -> Result<(), String> {
    let url = format!("{}/health", service_url.trim_end_matches('/'));
    let client = reqwest::blocking::Client::new();
    let started = Instant::now();
    while started.elapsed() < timeout {
        match docker_container_running(container_name) {
            Ok(true) => {}
            Ok(false) => {
                let logs = docker_logs_tail(container_name, 30);
                return Err(format!("SenseVoice 服务容器已退出。最近日志: {logs}"));
            }
            Err(err) => return Err(format!("读取服务容器状态失败: {err}")),
        }

        if let Ok(value) = client.get(&url).send() {
            if value.status().is_success() {
                let is_ready = value
                    .json::<serde_json::Value>()
                    .ok()
                    .and_then(|body| body.get("ready").and_then(|v| v.as_bool()))
                    .unwrap_or(false);
                if is_ready {
                    return Ok(());
                }
            }
        }
        thread::sleep(Duration::from_millis(500));
    }
    let logs = docker_logs_tail(container_name, 30);
    Err(format!(
        "SenseVoice 服务启动超时（{} 秒）。最近日志: {logs}",
        timeout.as_secs()
    ))
}

fn docker_logs_tail(container_name: &str, lines: usize) -> String {
    let mut command = docker_command();
    command
        .arg("logs")
        .arg("--tail")
        .arg(lines.to_string())
        .arg(container_name);
    hide_window(&mut command);
    let output = command.output();
    let Ok(output) = output else {
        return "（无日志）".to_string();
    };
    let mut merged = String::new();
    merged.push_str(&String::from_utf8_lossy(&output.stdout));
    merged.push_str(&String::from_utf8_lossy(&output.stderr));
    let text = merged.trim();
    if text.is_empty() {
        "（无日志）".to_string()
    } else {
        text.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(" || ")
    }
}

fn remove_container_if_exists(name: &str) -> Result<(), String> {
    let mut command = docker_command();
    command.arg("rm").arg("-f").arg(name);
    hide_window(&mut command);
    let output = command
        .output()
        .map_err(|err| format!("移除容器失败: {err}"))?;
    if output.status.success() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&output.stderr);
    if detail.contains("No such container") {
        return Ok(());
    }
    Err(format!("移除容器失败: {}", detail.trim()))
}

fn docker_container_running(name: &str) -> Result<bool, String> {
    let mut command = docker_command();
    command
        .arg("inspect")
        .arg("-f")
        .arg("{{.State.Running}}")
        .arg(name);
    hide_window(&mut command);
    let output = command
        .output()
        .map_err(|err| format!("检查容器状态失败: {err}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        if detail.contains("No such object") || detail.contains("No such container") {
            return Ok(false);
        }
        return Err(detail.trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim() == "true")
}

fn run_command_streaming<F>(
    command: &mut Command,
    step: &str,
    timeout: Duration,
    mut on_line: F,
) -> Result<(), String>
where
    F: FnMut(&str),
{
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    hide_window(command);

    let mut child = command.spawn().map_err(|err| format!("{step}失败: {err}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("{step}失败: 无法读取 stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| format!("{step}失败: 无法读取 stderr"))?;

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
            return Err(format!(
                "{step}超时（{} 秒），请检查网络或重试。最近日志: {}",
                timeout.as_secs(),
                tail.into_iter().collect::<Vec<_>>().join(" || ")
            ));
        }

        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => thread::sleep(Duration::from_millis(200)),
            Err(err) => {
                let _ = stdout_handle.join();
                let _ = stderr_handle.join();
                return Err(format!("{step}失败: {err}"));
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

    Err(format!(
        "{step}失败: {}",
        tail.into_iter().collect::<Vec<_>>().join(" || ")
    ))
}

fn parse_host_and_port(service_url: &str) -> Result<(String, u16), String> {
    let parsed = Url::parse(service_url).map_err(|err| format!("服务地址无效: {err}"))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| "服务地址缺少主机名".to_string())?
        .to_string();
    let port = parsed
        .port_or_known_default()
        .ok_or_else(|| "服务地址缺少端口".to_string())?;
    Ok((host, port))
}

fn normalize_publish_host(host: &str) -> Result<String, String> {
    if host.eq_ignore_ascii_case("localhost") {
        return Ok("127.0.0.1".to_string());
    }
    if host.parse::<Ipv4Addr>().is_ok() {
        return Ok(host.to_string());
    }
    Err("Docker 模式下服务地址主机仅支持 localhost 或 IPv4 地址".to_string())
}

fn read_selected_hub(state_file: &Path) -> Option<String> {
    let data = fs::read_to_string(state_file).ok()?;
    let value: serde_json::Value = serde_json::from_str(&data).ok()?;
    value
        .get("hub")
        .and_then(|item| item.as_str())
        .map(str::to_string)
}

fn bind_mount(source: &Path, target: &str) -> String {
    format!(
        "type=bind,source={},target={target}",
        source.to_string_lossy()
    )
}

fn normalize_log_line(line: &str) -> String {
    line.replace('\r', "").trim().to_string()
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

fn emit_progress(stage: &str, message: &str, percent: Option<u8>, detail: Option<String>) {
    emit_event(&WorkerEvent::Progress {
        stage: stage.to_string(),
        message: message.to_string(),
        percent,
        detail,
    });
}

fn emit_state(
    download_state: &str,
    last_error: &str,
    installed: Option<bool>,
    enabled: Option<bool>,
) {
    emit_event(&WorkerEvent::State {
        download_state: download_state.to_string(),
        last_error: last_error.to_string(),
        installed,
        enabled,
    });
}

fn emit_event(event: &WorkerEvent) {
    if let Ok(text) = serde_json::to_string(event) {
        println!("{text}");
        let _ = std::io::stdout().flush();
    }
}

pub fn parse_job_file_arg(args: &[String]) -> Option<String> {
    let mut index = 0usize;
    while index < args.len() {
        if args[index] == "--job-file" {
            return args.get(index + 1).cloned();
        }
        index += 1;
    }
    None
}
