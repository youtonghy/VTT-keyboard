use std::collections::VecDeque;
use std::fs;
use std::io::BufRead;
use std::net::Ipv4Addr;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use url::Url;

pub(super) fn docker_command() -> Command {
    Command::new("docker")
}

pub(super) fn hide_window(_command: &mut Command) {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        _command.creation_flags(0x0800_0000);
    }
}

pub(super) fn bind_mount(source: &Path, target: &str) -> String {
    format!(
        "type=bind,source={},target={target}",
        source.to_string_lossy()
    )
}

pub(super) fn normalize_log_line(line: &str) -> String {
    line.replace('\r', "").trim().to_string()
}

pub(super) fn docker_image_exists(image: &str) -> bool {
    let mut inspect = docker_command();
    inspect.arg("image").arg("inspect").arg(image);
    hide_window(&mut inspect);
    inspect.status().is_ok_and(|status| status.success())
}

pub(super) fn docker_container_running(name: &str) -> Result<bool, String> {
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

pub(super) fn remove_container_if_exists(name: &str) -> Result<(), String> {
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

pub(super) fn run_command_streaming<F>(
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

    let mut child = command
        .spawn()
        .map_err(|err| format!("{step}失败: {err}"))?;

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
        let reader = std::io::BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(value) = line {
                let _ = tx_out.send(value);
            }
        }
    });
    let stderr_handle = thread::spawn(move || {
        let reader = std::io::BufReader::new(stderr);
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

pub(super) fn parse_host_and_port(service_url: &str) -> Result<(String, u16), String> {
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

pub(super) fn normalize_publish_host(host: &str) -> Result<String, String> {
    if host.eq_ignore_ascii_case("localhost") {
        return Ok("127.0.0.1".to_string());
    }
    if host.parse::<Ipv4Addr>().is_ok() {
        return Ok(host.to_string());
    }
    Err("Docker 模式下服务地址主机仅支持 localhost 或 IPv4 地址".to_string())
}

pub(super) fn read_selected_hub(state_file: &Path) -> Option<String> {
    let data = fs::read_to_string(state_file).ok()?;
    let value: serde_json::Value = serde_json::from_str(&data).ok()?;
    value
        .get("hub")
        .and_then(|item| item.as_str())
        .map(str::to_string)
}
