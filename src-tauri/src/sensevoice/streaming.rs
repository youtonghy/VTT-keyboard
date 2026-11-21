//! 流式本地识别会话：录音期间边收音频边解码，并在状态浮窗里实时显示识别文字。
//!
//! 生命周期由 lib.rs 的录音命令驱动：
//! - 开始录音：`try_start` 创建会话，把返回的旁路交给录音器；
//! - 停止录音：`mark_stopped` 后把会话交给转写调度线程，由它调用 `finish` 取最终文字；
//! - 取消录音：直接丢弃会话（`Drop` 会标记取消，后台线程不再输出任何内容）。

use super::model::spec_for_local_model;
use super::{native_runtime, sherpa, SenseVoiceError};
use crate::recorder::{AudioTap, TapMessage};
use crate::settings::{Settings, TranscriptionProvider};
use sherpa_onnx::OnlineRecognizer;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// 浮窗文字的最短刷新间隔
const PREVIEW_INTERVAL: Duration = Duration::from_millis(200);
/// 录音停止后，超过这么久没有新音频即认为输入结束。
/// 不依赖录音器释放旁路，避免个别平台延迟销毁音频回调时最终结果一直等不到。
const INPUT_IDLE_AFTER_STOP: Duration = Duration::from_millis(100);
/// 浮窗单行可显示的宽度（汉字计 2，其余字符计 1）
const PREVIEW_MAX_WIDTH: usize = 48;

pub struct StreamingSession {
    stopped: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
    worker: Option<JoinHandle<Result<String, SenseVoiceError>>>,
}

/// 当前选择的是已加载的流式本地模型时创建会话；否则返回 None，走整段识别。
/// 模型未加载时不在这里同步加载，避免按下快捷键时卡住。
pub fn try_start(settings: &Settings) -> Option<(AudioTap, StreamingSession)> {
    if settings.provider != TranscriptionProvider::Sensevoice || !settings.sensevoice.enabled {
        return None;
    }
    let spec = spec_for_local_model(&settings.sensevoice.local_model);
    if !spec.kind.is_streaming() {
        return None;
    }
    let models_root = native_runtime::get_models_root()?;
    if !sherpa::is_loaded(&models_root, spec.model_key) {
        return None;
    }
    let recognizer = match sherpa::online_recognizer(&models_root, spec.model_key) {
        Ok(value) => value,
        Err(_err) => {
            #[cfg(debug_assertions)]
            eprintln!("流式识别器不可用，改为整段识别: {_err}");
            return None;
        }
    };
    Some(start_with_recognizer(recognizer, |text| {
        crate::processing::emit_status_detail("recording", text)
    }))
}

fn start_with_recognizer<F>(
    recognizer: Arc<OnlineRecognizer>,
    on_preview: F,
) -> (AudioTap, StreamingSession)
where
    F: Fn(&str) + Send + 'static,
{
    let (tap, receiver) = mpsc::channel::<TapMessage>();
    let stopped = Arc::new(AtomicBool::new(false));
    let cancelled = Arc::new(AtomicBool::new(false));
    let worker = {
        let stopped = stopped.clone();
        let cancelled = cancelled.clone();
        thread::spawn(move || run(&recognizer, receiver, &stopped, &cancelled, on_preview))
    };
    (
        tap,
        StreamingSession {
            stopped,
            cancelled,
            worker: Some(worker),
        },
    )
}

impl StreamingSession {
    /// 录音已停止：此后不再刷新浮窗（避免覆盖“正在转写”状态）
    pub fn mark_stopped(&self) {
        self.stopped.store(true, Ordering::SeqCst);
    }

    /// 等待剩余音频解码完毕并返回最终文字。调用前录音器必须已停止，
    /// 否则旁路不会关闭，此调用会一直等待。
    pub fn finish(mut self) -> Result<String, SenseVoiceError> {
        self.mark_stopped();
        let worker = self
            .worker
            .take()
            .ok_or_else(|| SenseVoiceError::Process("流式会话已结束".to_string()))?;
        worker
            .join()
            .map_err(|_| SenseVoiceError::Process("流式识别线程异常退出".to_string()))?
    }
}

impl Drop for StreamingSession {
    fn drop(&mut self) {
        // 未调用 finish 就被丢弃（取消录音、启动失败）：让后台线程静默退出
        if self.worker.is_some() {
            self.cancelled.store(true, Ordering::SeqCst);
            self.stopped.store(true, Ordering::SeqCst);
        }
    }
}

fn run<F>(
    recognizer: &OnlineRecognizer,
    receiver: mpsc::Receiver<TapMessage>,
    stopped: &AtomicBool,
    cancelled: &AtomicBool,
    on_preview: F,
) -> Result<String, SenseVoiceError>
where
    F: Fn(&str),
{
    let stream = recognizer.create_stream();
    let mut format: Option<(i32, usize)> = None;
    let mut shown = String::new();
    let mut last_preview = Instant::now() - PREVIEW_INTERVAL;

    loop {
        let message = match receiver.recv_timeout(INPUT_IDLE_AFTER_STOP) {
            Ok(message) => message,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if stopped.load(Ordering::SeqCst) {
                    break;
                }
                continue;
            }
            // 录音器释放了旁路：音频已全部送达
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        };
        if cancelled.load(Ordering::SeqCst) {
            return Err(SenseVoiceError::Process("流式识别已取消".to_string()));
        }
        match message {
            TapMessage::Format {
                sample_rate,
                channels,
            } => {
                let rate = sample_rate as i32;
                // 开头补静音，避免流式模型丢掉第一个词
                stream.accept_waveform(
                    rate,
                    &sherpa::silence(rate, sherpa::ONLINE_LEAD_SILENCE_SECONDS),
                );
                format = Some((rate, usize::from(channels.max(1))));
            }
            TapMessage::Samples(samples) => {
                let Some((rate, channels)) = format else {
                    continue;
                };
                stream.accept_waveform(rate, &to_mono_f32(&samples, channels));
                sherpa::decode_ready(recognizer, &stream);
                if stopped.load(Ordering::SeqCst) || last_preview.elapsed() < PREVIEW_INTERVAL {
                    continue;
                }
                let text = sherpa::online_text(recognizer, &stream);
                let preview = preview_tail(&text);
                if !preview.is_empty() && preview != shown {
                    on_preview(&preview);
                    shown = preview;
                    last_preview = Instant::now();
                }
            }
        }
    }

    if cancelled.load(Ordering::SeqCst) {
        return Err(SenseVoiceError::Process("流式识别已取消".to_string()));
    }
    let (rate, _) =
        format.ok_or_else(|| SenseVoiceError::Process("流式识别没有收到音频".to_string()))?;
    Ok(sherpa::finish_online_stream(recognizer, &stream, rate))
}

fn to_mono_f32(samples: &[i16], channels: usize) -> Vec<f32> {
    let floats = samples
        .iter()
        .map(|value| f32::from(*value) / f32::from(i16::MAX))
        .collect();
    sherpa::downmix_to_mono(floats, channels)
}

fn char_width(ch: char) -> usize {
    if ch.is_ascii() {
        1
    } else {
        2
    }
}

/// 浮窗只有一行：保留文字末尾能放下的部分，被截断时前面加省略号
fn preview_tail(text: &str) -> String {
    let text = text.trim();
    let total: usize = text.chars().map(char_width).sum();
    if total <= PREVIEW_MAX_WIDTH {
        return text.to_string();
    }
    let budget = PREVIEW_MAX_WIDTH - 1;
    let mut width = 0;
    let mut tail: Vec<char> = Vec::new();
    for ch in text.chars().rev() {
        let next = width + char_width(ch);
        if next > budget {
            break;
        }
        width = next;
        tail.push(ch);
    }
    let tail: String = tail.into_iter().rev().collect();
    format!("…{}", tail.trim_start())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_preview_is_unchanged() {
        assert_eq!(preview_tail("  hello world "), "hello world");
        assert_eq!(preview_tail(""), "");
    }

    #[test]
    fn long_english_preview_keeps_the_tail() {
        let text = "please schedule a meeting with the design team for tomorrow afternoon";
        let preview = preview_tail(text);
        assert!(preview.starts_with('…'));
        assert!(preview.ends_with("tomorrow afternoon"));
        let width: usize = preview.chars().map(char_width).sum();
        assert!(width <= PREVIEW_MAX_WIDTH + 1, "{preview}");
    }

    #[test]
    fn long_chinese_preview_counts_double_width() {
        let text = "今天下午我们在会议室讨论新版本的发布计划请大家准时参加并提前准备好材料";
        let preview = preview_tail(text);
        assert!(preview.starts_with('…'));
        assert!(preview.ends_with("提前准备好材料"));
        let chars = preview.chars().count() - 1;
        assert!(chars <= PREVIEW_MAX_WIDTH / 2, "{preview}");
    }

    /// 用本地已下载的流式模型模拟一次录音（默认忽略）：
    /// VTT_MODELS_ROOT=<模型目录> VTT_TEST_WAV=<16-bit 单声道 WAV> VTT_TEST_MODEL=<流式 model key>
    /// cargo test --lib -- --ignored --nocapture
    #[test]
    #[ignore]
    fn streaming_session_transcribes_wav() {
        let root = std::path::PathBuf::from(std::env::var("VTT_MODELS_ROOT").unwrap());
        let wav = std::env::var("VTT_TEST_WAV").unwrap();
        let model = std::env::var("VTT_TEST_MODEL").unwrap();
        let recognizer = sherpa::online_recognizer(&root, &model).expect("load model");
        let previews = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = previews.clone();
        let (tap, session) = start_with_recognizer(recognizer, move |text| {
            sink.lock().unwrap().push(text.to_string())
        });
        let mut reader = hound::WavReader::open(&wav).unwrap();
        let spec = reader.spec();
        let samples: Vec<i16> = reader.samples::<i16>().map(|s| s.unwrap()).collect();
        tap.send(TapMessage::Format {
            sample_rate: spec.sample_rate,
            channels: spec.channels,
        })
        .unwrap();
        // 按 100ms 实时节奏送入，模拟录音
        let chunk = spec.sample_rate as usize / 10 * usize::from(spec.channels);
        for piece in samples.chunks(chunk) {
            tap.send(TapMessage::Samples(piece.to_vec())).unwrap();
            thread::sleep(Duration::from_millis(100));
        }
        // 故意在 finish 之后才释放旁路，模拟平台延迟销毁音频回调
        let started = Instant::now();
        let text = session.finish().expect("finish");
        drop(tap);
        println!("release latency: {:?}", started.elapsed());
        println!("previews: {:?}", previews.lock().unwrap());
        println!("final: {text}");
        assert!(!text.trim().is_empty());
        assert!(!previews.lock().unwrap().is_empty());
    }

    #[test]
    fn stereo_is_downmixed() {
        assert_eq!(to_mono_f32(&[i16::MAX, 0, 0, 0], 2), vec![0.5, 0.0]);
    }
}
