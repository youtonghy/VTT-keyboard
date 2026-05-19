use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, FromSample, Sample, SampleFormat, Stream, StreamConfig};
use serde::Serialize;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use thiserror::Error;

#[cfg(target_os = "macos")]
#[link(name = "AVFoundation", kind = "framework")]
extern "C" {}

#[cfg(target_os = "macos")]
extern "C" {
    fn macos_microphone_permission_status_code() -> i32;
}

#[derive(Debug, Error)]
pub enum RecorderError {
    #[error("无法获取默认输入设备")]
    DeviceUnavailable,
    #[error("无法找到输入设备: {0}")]
    DeviceNotFound(String),
    #[error("无法枚举输入设备: {0}")]
    DeviceQuery(String),
    #[error("无法获取输入配置: {0}")]
    Config(String),
    #[error("无法启动录音: {0}")]
    Stream(String),
    #[error("录音尚未开始")]
    NotRecording,
    #[error("录音状态锁异常")]
    LockPoisoned,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioInputDevice {
    pub name: String,
    pub is_default: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioInputTestResult {
    pub peak_level: f32,
    pub average_level: f32,
    pub sample_count: usize,
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MicrophonePermissionStatus {
    pub status: String,
    pub supported: bool,
}

#[derive(Clone)]
pub struct Recorder {
    inner: Arc<Mutex<RecorderInner>>,
}

struct RecorderInner {
    stream: Option<Stream>,
    buffer: Arc<Mutex<Vec<i16>>>,
    config: Option<StreamConfig>,
}

pub struct RecorderService {
    sender: mpsc::Sender<RecorderCommand>,
}

enum RecorderCommand {
    Start {
        input_device_name: Option<String>,
        reply: mpsc::Sender<Result<(), RecorderError>>,
    },
    Stop(mpsc::Sender<Result<RecordedAudio, RecorderError>>),
}

impl RecorderService {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel::<RecorderCommand>();
        std::thread::spawn(move || {
            let recorder = Recorder::new();
            loop {
                match receiver.recv() {
                    Ok(RecorderCommand::Start {
                        input_device_name,
                        reply,
                    }) => {
                        let result = recorder.start(input_device_name.as_deref());
                        let _ = reply.send(result);
                    }
                    Ok(RecorderCommand::Stop(reply)) => {
                        let result = recorder.stop();
                        let _ = reply.send(result);
                    }
                    Err(_) => break,
                }
            }
        });
        Self { sender }
    }

    pub fn start(&self, input_device_name: Option<String>) -> Result<(), RecorderError> {
        let (reply_tx, reply_rx) = mpsc::channel();
        let _ = self.sender.send(RecorderCommand::Start {
            input_device_name,
            reply: reply_tx,
        });
        reply_rx.recv().unwrap_or(Err(RecorderError::NotRecording))
    }

    pub fn stop(&self) -> Result<RecordedAudio, RecorderError> {
        let (reply_tx, reply_rx) = mpsc::channel();
        let _ = self.sender.send(RecorderCommand::Stop(reply_tx));
        reply_rx.recv().unwrap_or(Err(RecorderError::NotRecording))
    }
}

impl Recorder {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(RecorderInner {
                stream: None,
                buffer: Arc::new(Mutex::new(Vec::new())),
                config: None,
            })),
        }
    }

    pub fn start(&self, input_device_name: Option<&str>) -> Result<(), RecorderError> {
        let inner = self.inner.lock().map_err(|_| RecorderError::LockPoisoned)?;
        if inner.stream.is_some() {
            return Ok(());
        }
        drop(inner);

        let host = cpal::default_host();
        let device = resolve_input_device(&host, input_device_name)?;
        let input_config = device
            .default_input_config()
            .map_err(|err| RecorderError::Config(err.to_string()))?;
        let config: StreamConfig = input_config.clone().into();

        let buffer = Arc::new(Mutex::new(Vec::new()));
        let buffer_clone = buffer.clone();
        let err_fn = |_err| {
            #[cfg(debug_assertions)]
            eprintln!("录音流错误: {_err}");
        };

        macro_rules! build_stream {
            ($sample_type:ty) => {
                device
                    .build_input_stream(
                        &config,
                        move |data: &[$sample_type], _| push_samples(data, &buffer_clone),
                        err_fn,
                        None,
                    )
                    .map_err(|err| RecorderError::Stream(err.to_string()))?
            };
        }

        let stream = match input_config.sample_format() {
            SampleFormat::I16 => build_stream!(i16),
            SampleFormat::U16 => build_stream!(u16),
            SampleFormat::F32 => build_stream!(f32),
            _ => return Err(RecorderError::Config("不支持的采样格式".to_string())),
        };

        stream
            .play()
            .map_err(|err| RecorderError::Stream(err.to_string()))?;

        let mut inner = self.inner.lock().map_err(|_| RecorderError::LockPoisoned)?;
        inner.stream = Some(stream);
        inner.buffer = buffer;
        inner.config = Some(config);
        Ok(())
    }

    pub fn stop(&self) -> Result<RecordedAudio, RecorderError> {
        let mut inner = self.inner.lock().map_err(|_| RecorderError::LockPoisoned)?;
        let Some(config) = inner.config.clone() else {
            return Err(RecorderError::NotRecording);
        };
        let buffer = inner
            .buffer
            .lock()
            .map_err(|_| RecorderError::LockPoisoned)?
            .clone();
        inner.stream.take();
        inner.config = None;
        Ok(RecordedAudio {
            samples: buffer,
            sample_rate: config.sample_rate.0,
            channels: config.channels,
        })
    }
}

#[derive(Clone)]
pub struct RecordedAudio {
    pub samples: Vec<i16>,
    pub sample_rate: u32,
    pub channels: u16,
}

fn push_samples<T>(data: &[T], buffer: &Arc<Mutex<Vec<i16>>>)
where
    T: Sample,
    i16: FromSample<T>,
{
    if let Ok(mut guard) = buffer.lock() {
        guard.extend(data.iter().map(|sample| i16::from_sample(*sample)));
    }
}

pub fn list_input_devices() -> Result<Vec<AudioInputDevice>, RecorderError> {
    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .and_then(|device| device.name().ok());
    let devices = host
        .input_devices()
        .map_err(|err| RecorderError::DeviceQuery(err.to_string()))?;

    let mut result = Vec::new();
    for device in devices {
        let name = device
            .name()
            .map_err(|err| RecorderError::DeviceQuery(err.to_string()))?;
        if result
            .iter()
            .any(|item: &AudioInputDevice| item.name == name)
        {
            continue;
        }
        result.push(AudioInputDevice {
            is_default: default_name.as_deref() == Some(name.as_str()),
            name,
        });
    }
    Ok(result)
}

pub fn microphone_permission_status() -> MicrophonePermissionStatus {
    #[cfg(target_os = "macos")]
    {
        return MicrophonePermissionStatus {
            status: macos_microphone_permission_status(),
            supported: true,
        };
    }

    #[cfg(not(target_os = "macos"))]
    {
        MicrophonePermissionStatus {
            status: "unsupported".to_string(),
            supported: false,
        }
    }
}

pub fn test_input_device(
    input_device_name: Option<&str>,
    duration_ms: u64,
) -> Result<AudioInputTestResult, RecorderError> {
    let host = cpal::default_host();
    let device = resolve_input_device(&host, input_device_name)?;
    let input_config = device
        .default_input_config()
        .map_err(|err| RecorderError::Config(err.to_string()))?;
    let config: StreamConfig = input_config.clone().into();
    let buffer = Arc::new(Mutex::new(Vec::new()));
    let buffer_clone = buffer.clone();
    let err_fn = |_err| {
        #[cfg(debug_assertions)]
        eprintln!("麦克风测试流错误: {_err}");
    };

    macro_rules! build_stream {
        ($sample_type:ty) => {
            device
                .build_input_stream(
                    &config,
                    move |data: &[$sample_type], _| push_samples(data, &buffer_clone),
                    err_fn,
                    None,
                )
                .map_err(|err| RecorderError::Stream(err.to_string()))?
        };
    }

    let stream = match input_config.sample_format() {
        SampleFormat::I16 => build_stream!(i16),
        SampleFormat::U16 => build_stream!(u16),
        SampleFormat::F32 => build_stream!(f32),
        _ => return Err(RecorderError::Config("不支持的采样格式".to_string())),
    };

    stream
        .play()
        .map_err(|err| RecorderError::Stream(err.to_string()))?;
    std::thread::sleep(Duration::from_millis(duration_ms.clamp(80, 3000)));
    drop(stream);

    let samples = buffer.lock().map_err(|_| RecorderError::LockPoisoned)?;
    Ok(AudioInputTestResult {
        peak_level: calculate_peak_level(&samples),
        average_level: calculate_average_level(&samples),
        sample_count: samples.len(),
        sample_rate: config.sample_rate.0,
        channels: config.channels,
    })
}

fn resolve_input_device(
    host: &cpal::Host,
    input_device_name: Option<&str>,
) -> Result<Device, RecorderError> {
    let requested = input_device_name
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(requested_name) = requested else {
        return host
            .default_input_device()
            .ok_or(RecorderError::DeviceUnavailable);
    };

    let devices = host
        .input_devices()
        .map_err(|err| RecorderError::DeviceQuery(err.to_string()))?;
    for device in devices {
        let name = device
            .name()
            .map_err(|err| RecorderError::DeviceQuery(err.to_string()))?;
        if name == requested_name {
            return Ok(device);
        }
    }
    Err(RecorderError::DeviceNotFound(requested_name.to_string()))
}

fn calculate_peak_level(samples: &[i16]) -> f32 {
    samples
        .iter()
        .map(|sample| sample.unsigned_abs() as f32 / i16::MAX as f32)
        .fold(0.0, f32::max)
        .clamp(0.0, 1.0)
}

fn calculate_average_level(samples: &[i16]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum = samples
        .iter()
        .map(|sample| sample.unsigned_abs() as f64 / i16::MAX as f64)
        .sum::<f64>();
    (sum / samples.len() as f64).clamp(0.0, 1.0) as f32
}

#[cfg(target_os = "macos")]
fn macos_microphone_permission_status() -> String {
    let status = unsafe { macos_microphone_permission_status_code() };
    match status {
        0 => "notDetermined",
        1 => "restricted",
        2 => "denied",
        3 => "authorized",
        _ => "unknown",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::{calculate_average_level, calculate_peak_level};

    #[test]
    fn input_level_calculation_handles_empty_and_signal_samples() {
        assert_eq!(calculate_peak_level(&[]), 0.0);
        assert_eq!(calculate_average_level(&[]), 0.0);

        let samples = [0, i16::MAX / 2, -i16::MAX];
        let peak = calculate_peak_level(&samples);
        let average = calculate_average_level(&samples);

        assert!((peak - 1.0).abs() < f32::EPSILON);
        assert!(average > 0.49 && average < 0.51);
    }
}
