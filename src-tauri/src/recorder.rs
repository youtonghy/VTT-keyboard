use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, Stream, StreamConfig};
use std::sync::{Arc, Mutex, mpsc};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RecorderError {
    #[error("无法获取默认输入设备")]
    DeviceUnavailable,
    #[error("无法获取输入配置: {0}")]
    Config(String),
    #[error("无法启动录音: {0}")]
    Stream(String),
    #[error("录音尚未开始")]
    NotRecording,
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
    Start(mpsc::Sender<Result<(), RecorderError>>),
    Stop(mpsc::Sender<Result<RecordedAudio, RecorderError>>),
}

impl RecorderService {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel::<RecorderCommand>();
        std::thread::spawn(move || {
            let recorder = Recorder::new();
            loop {
                match receiver.recv() {
                    Ok(RecorderCommand::Start(reply)) => {
                        let result = recorder.start();
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

    pub fn start(&self) -> Result<(), RecorderError> {
        let (reply_tx, reply_rx) = mpsc::channel();
        let _ = self.sender.send(RecorderCommand::Start(reply_tx));
        reply_rx.recv().unwrap_or_else(|_| Err(RecorderError::NotRecording))
    }

    pub fn stop(&self) -> Result<RecordedAudio, RecorderError> {
        let (reply_tx, reply_rx) = mpsc::channel();
        let _ = self.sender.send(RecorderCommand::Stop(reply_tx));
        reply_rx.recv().unwrap_or_else(|_| Err(RecorderError::NotRecording))
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

    pub fn start(&self) -> Result<(), RecorderError> {
        let inner = self.inner.lock().expect("录音锁错误");
        if inner.stream.is_some() {
            return Ok(());
        }
        drop(inner);

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or(RecorderError::DeviceUnavailable)?;
        let input_config = device
            .default_input_config()
            .map_err(|err| RecorderError::Config(err.to_string()))?;
        let config: StreamConfig = input_config.clone().into();

        let buffer = Arc::new(Mutex::new(Vec::new()));
        let buffer_clone = buffer.clone();
        let err_fn = |err| eprintln!("录音流错误: {err}");

        let stream = match input_config.sample_format() {
            SampleFormat::I16 => device
                .build_input_stream(
                    &config,
                    move |data: &[i16], _| push_samples(data, &buffer_clone),
                    err_fn,
                    None,
                )
                .map_err(|err| RecorderError::Stream(err.to_string()))?,
            SampleFormat::U16 => device
                .build_input_stream(
                    &config,
                    move |data: &[u16], _| push_samples(data, &buffer_clone),
                    err_fn,
                    None,
                )
                .map_err(|err| RecorderError::Stream(err.to_string()))?,
            SampleFormat::F32 => device
                .build_input_stream(
                    &config,
                    move |data: &[f32], _| push_samples(data, &buffer_clone),
                    err_fn,
                    None,
                )
                .map_err(|err| RecorderError::Stream(err.to_string()))?,
            _ => return Err(RecorderError::Config("不支持的采样格式".to_string())),
        };

        stream
            .play()
            .map_err(|err| RecorderError::Stream(err.to_string()))?;

        let mut inner = self.inner.lock().expect("录音锁错误");
        inner.stream = Some(stream);
        inner.buffer = buffer;
        inner.config = Some(config);
        Ok(())
    }

    pub fn stop(&self) -> Result<RecordedAudio, RecorderError> {
        let mut inner = self.inner.lock().expect("录音锁错误");
        let Some(config) = inner.config.clone() else {
            return Err(RecorderError::NotRecording);
        };
        let buffer = inner.buffer.lock().expect("录音缓冲锁错误").clone();
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
    let mut guard = buffer.lock().expect("录音缓冲锁错误");
    guard.extend(data.iter().map(|sample| i16::from_sample(*sample)));
}
