use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, FromSample, Sample, SampleFormat, Stream, StreamConfig};
use serde::Serialize;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use thiserror::Error;

use crate::permissions;

#[cfg(target_os = "macos")]
#[link(name = "AVFoundation", kind = "framework")]
extern "C" {}

#[cfg(target_os = "macos")]
extern "C" {
    fn macos_microphone_permission_status_code() -> i32;
    fn macos_request_microphone_permission_code() -> i32;
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
    #[error("麦克风权限未授权: {0}")]
    Permission(String),
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
    pub is_silent: bool,
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
    Abort(mpsc::Sender<Result<bool, RecorderError>>),
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
                    Ok(RecorderCommand::Abort(reply)) => {
                        let result = recorder.abort();
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

    pub fn abort(&self) -> Result<bool, RecorderError> {
        let (reply_tx, reply_rx) = mpsc::channel();
        let _ = self.sender.send(RecorderCommand::Abort(reply_tx));
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
        ensure_microphone_permission_already_authorized()?;

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
        let (stream, buffer, config) = {
            let mut inner = self.inner.lock().map_err(|_| RecorderError::LockPoisoned)?;
            let Some(config) = inner.config.take() else {
                return Err(RecorderError::NotRecording);
            };
            let stream = inner.stream.take();
            let buffer = inner.buffer.clone();
            (stream, buffer, config)
        };

        release_input_stream(stream);
        let mut guard = buffer.lock().map_err(|_| RecorderError::LockPoisoned)?;
        let samples = guard.clone();
        guard.clear();
        Ok(RecordedAudio {
            samples,
            sample_rate: config.sample_rate.0,
            channels: config.channels,
        })
    }

    pub fn abort(&self) -> Result<bool, RecorderError> {
        let (stream, buffer, was_recording) = {
            let mut inner = self.inner.lock().map_err(|_| RecorderError::LockPoisoned)?;
            let stream = inner.stream.take();
            let was_recording = stream.is_some() || inner.config.is_some();
            inner.config = None;
            let buffer = inner.buffer.clone();
            (stream, buffer, was_recording)
        };

        release_input_stream(stream);
        if let Ok(mut guard) = buffer.lock() {
            guard.clear();
        }
        Ok(was_recording)
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

pub fn request_microphone_permission() -> MicrophonePermissionStatus {
    #[cfg(target_os = "macos")]
    {
        return MicrophonePermissionStatus {
            status: permission_status_from_code(unsafe {
                macos_request_microphone_permission_code()
            })
            .to_string(),
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
    ensure_microphone_permission_already_authorized()?;

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
    release_input_stream(Some(stream));

    let samples = buffer.lock().map_err(|_| RecorderError::LockPoisoned)?;
    let peak_level = calculate_peak_level(&samples);
    let average_level = calculate_average_level(&samples);
    let sample_count = samples.len();
    Ok(AudioInputTestResult {
        peak_level,
        average_level,
        sample_count,
        sample_rate: config.sample_rate.0,
        channels: config.channels,
        is_silent: sample_count > 0 && peak_level <= f32::EPSILON && average_level <= f32::EPSILON,
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

fn release_input_stream(stream: Option<Stream>) {
    let Some(stream) = stream else {
        return;
    };

    if let Err(_err) = stream.pause() {
        #[cfg(debug_assertions)]
        eprintln!("停止输入流失败，将继续释放资源: {_err}");
    }
    drop(stream);

    #[cfg(target_os = "macos")]
    std::thread::sleep(Duration::from_millis(30));
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
    permission_status_from_code(unsafe { macos_microphone_permission_status_code() }).to_string()
}

fn permission_status_from_code(status: i32) -> &'static str {
    match status {
        0 => "notDetermined",
        1 => "restricted",
        2 => "denied",
        3 => "authorized",
        _ => "unknown",
    }
}

fn ensure_microphone_permission_already_authorized() -> Result<(), RecorderError> {
    #[cfg(target_os = "macos")]
    {
        return match permissions::missing_recording_permission_message() {
            Some(message) => Err(RecorderError::Permission(message)),
            None => Ok(()),
        };
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        calculate_average_level, calculate_peak_level, permission_status_from_code, Recorder,
        RecorderError,
    };
    use cpal::{BufferSize, SampleRate, StreamConfig};

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

    #[test]
    fn microphone_permission_status_code_maps_expected_values() {
        assert_eq!(permission_status_from_code(0), "notDetermined");
        assert_eq!(permission_status_from_code(1), "restricted");
        assert_eq!(permission_status_from_code(2), "denied");
        assert_eq!(permission_status_from_code(3), "authorized");
        assert_eq!(permission_status_from_code(-1), "unknown");
    }

    #[test]
    fn abort_is_idempotent_when_recorder_is_idle() {
        let recorder = Recorder::new();

        assert!(!recorder.abort().expect("idle abort should succeed"));
        assert!(!recorder
            .abort()
            .expect("repeated idle abort should succeed"));
        assert!(matches!(recorder.stop(), Err(RecorderError::NotRecording)));
    }

    #[test]
    fn stop_clears_recording_state_and_returns_samples() {
        let recorder = Recorder::new();
        {
            let mut inner = recorder.inner.lock().expect("recorder lock");
            inner.buffer = std::sync::Arc::new(std::sync::Mutex::new(vec![1, -2, 3]));
            inner.config = Some(test_stream_config());
        }

        let audio = recorder.stop().expect("recording should stop");

        assert_eq!(audio.samples, vec![1, -2, 3]);
        assert_eq!(audio.sample_rate, 16_000);
        assert_eq!(audio.channels, 1);

        let inner = recorder.inner.lock().expect("recorder lock");
        assert!(inner.stream.is_none());
        assert!(inner.config.is_none());
        assert!(inner.buffer.lock().expect("buffer lock").is_empty());
    }

    #[test]
    fn abort_clears_recording_state_and_buffer() {
        let recorder = Recorder::new();
        {
            let mut inner = recorder.inner.lock().expect("recorder lock");
            inner.buffer = std::sync::Arc::new(std::sync::Mutex::new(vec![10, 20]));
            inner.config = Some(test_stream_config());
        }

        assert!(recorder.abort().expect("recording should abort"));

        let inner = recorder.inner.lock().expect("recorder lock");
        assert!(inner.stream.is_none());
        assert!(inner.config.is_none());
        assert!(inner.buffer.lock().expect("buffer lock").is_empty());
    }

    fn test_stream_config() -> StreamConfig {
        StreamConfig {
            channels: 1,
            sample_rate: SampleRate(16_000),
            buffer_size: BufferSize::Default,
        }
    }
}
