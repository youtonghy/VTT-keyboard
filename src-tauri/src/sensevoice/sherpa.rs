use super::model::{download_urls, spec_for_local_model, LocalModelKind, LocalModelSpec};
use crate::sensevoice::SenseVoiceError;
use crate::settings::TranscriptionAlignment;
use bzip2::read::BzDecoder;
use reqwest::blocking::Client;
use sherpa_onnx::{
    OfflineFireRedAsrCtcModelConfig, OfflineFireRedAsrModelConfig, OfflineFunASRNanoModelConfig,
    OfflineMoonshineModelConfig, OfflineParaformerModelConfig, OfflineRecognizer,
    OfflineRecognizerConfig, OfflineSenseVoiceModelConfig, OfflineTransducerModelConfig,
    OnlineRecognizer, OnlineRecognizerConfig, OnlineStream, OnlineTransducerModelConfig,
};
use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use tar::Archive;

const TOKENS_FILE_NAME: &str = "tokens.txt";
const FUNASR_NANO_TOKENIZER_DIR: &str = "Qwen3-0.6B";
const RECOGNIZER_THREADS: i32 = 2;

/// 流式模型在开头补的静音秒数：音频零前导静音时 Nemotron 会丢掉第一个词
pub const ONLINE_LEAD_SILENCE_SECONDS: f32 = 0.3;
/// 流式模型在输入结束后补的静音秒数，保证最后一个分块被完整解码
const ONLINE_TAIL_PADDING_SECONDS: f32 = 0.6;

struct SherpaRuntime {
    model_dir: PathBuf,
    language: String,
    /// 流式模型的识别器常驻内存；离线模型每次转写时重建
    online: Option<Arc<OnlineRecognizer>>,
}

static SHERPA_RUNTIME: OnceLock<Mutex<Option<SherpaRuntime>>> = OnceLock::new();
static SHERPA_MODELS_ROOT: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();

fn runtime_slot() -> &'static Mutex<Option<SherpaRuntime>> {
    SHERPA_RUNTIME.get_or_init(|| Mutex::new(None))
}

fn models_root_slot() -> &'static Mutex<Option<PathBuf>> {
    SHERPA_MODELS_ROOT.get_or_init(|| Mutex::new(None))
}

pub fn set_models_root(models_root: PathBuf) {
    if let Ok(mut slot) = models_root_slot().lock() {
        *slot = Some(models_root);
    }
}

pub fn get_models_root() -> Option<PathBuf> {
    models_root_slot().lock().ok().and_then(|slot| slot.clone())
}

pub fn runtime_model_dir(models_root: &Path, spec: &LocalModelSpec) -> PathBuf {
    models_root.join(spec.model_id)
}

pub fn model_is_ready(models_root: &Path, spec: &LocalModelSpec) -> bool {
    let model_dir = runtime_model_dir(models_root, spec);
    spec.required_files
        .iter()
        .all(|file| model_dir.join(file).exists())
}

pub fn prepare_model<F>(
    models_root: &Path,
    local_model: &str,
    mut on_progress: F,
) -> Result<(), SenseVoiceError>
where
    F: FnMut(&str, Option<u8>, Option<u64>, Option<u64>),
{
    let spec = spec_for_local_model(local_model);
    if model_is_ready(models_root, spec) {
        return Ok(());
    }

    fs::create_dir_all(models_root).map_err(|err| SenseVoiceError::Io(err.to_string()))?;

    let archive_path = models_root.join(spec.archive_name);
    on_progress(
        &format!("Downloading {}", spec.archive_name),
        Some(0),
        Some(0),
        None,
    );
    download_archive(&archive_path, spec, &mut on_progress)?;
    on_progress(
        &format!("Extracting {} model", spec.display_name),
        Some(95),
        None,
        None,
    );
    extract_archive(&archive_path, models_root)?;
    let _ = fs::remove_file(&archive_path);

    if !model_is_ready(models_root, spec) {
        return Err(SenseVoiceError::Io("模型文件解压后不完整".to_string()));
    }
    Ok(())
}

pub fn load_runtime(
    models_root: &Path,
    local_model: &str,
    language: &str,
) -> Result<(), SenseVoiceError> {
    let spec = spec_for_local_model(local_model);
    let model_dir = runtime_model_dir(models_root, spec);
    if !model_is_ready(models_root, spec) {
        return Err(SenseVoiceError::Config(format!(
            "{} 模型尚未下载",
            spec.display_name
        )));
    }

    let normalized_language = normalize_language(spec, language);
    let mut runtime = runtime_slot()
        .lock()
        .map_err(|_| SenseVoiceError::Process("failed to lock Sherpa runtime".to_string()))?;
    let should_reload = runtime.as_ref().is_none_or(|current| {
        current.model_dir != model_dir || current.language != normalized_language
    });
    if !should_reload {
        return Ok(());
    }

    let online = if spec.kind.is_streaming() {
        Some(Arc::new(create_online_recognizer(&model_dir, spec)?))
    } else {
        let _ = create_recognizer(&model_dir, spec, &normalized_language)?;
        None
    };

    *runtime = Some(SherpaRuntime {
        model_dir,
        language: normalized_language,
        online,
    });
    Ok(())
}

/// 加载（如需要）并返回流式模型的常驻识别器
pub fn online_recognizer(
    models_root: &Path,
    local_model: &str,
) -> Result<Arc<OnlineRecognizer>, SenseVoiceError> {
    load_runtime(models_root, local_model, "auto")?;
    let runtime = runtime_slot()
        .lock()
        .map_err(|_| SenseVoiceError::Process("failed to lock Sherpa runtime".to_string()))?;
    runtime
        .as_ref()
        .and_then(|rt| rt.online.clone())
        .ok_or_else(|| SenseVoiceError::Process("流式识别器尚未加载".to_string()))
}

/// 一段静音样本
pub fn silence(sample_rate: i32, seconds: f32) -> Vec<f32> {
    vec![0.0; (sample_rate as f32 * seconds).max(0.0) as usize]
}

/// 解码流中已就绪的全部分块
pub fn decode_ready(recognizer: &OnlineRecognizer, stream: &OnlineStream) {
    while recognizer.is_ready(stream) {
        recognizer.decode(stream);
    }
}

/// 当前已识别出的文字
pub fn online_text(recognizer: &OnlineRecognizer, stream: &OnlineStream) -> String {
    recognizer
        .get_result(stream)
        .map(|result| result.text)
        .unwrap_or_default()
}

/// 结束输入：补尾部静音、解码剩余分块并返回最终文字
pub fn finish_online_stream(
    recognizer: &OnlineRecognizer,
    stream: &OnlineStream,
    sample_rate: i32,
) -> String {
    stream.accept_waveform(
        sample_rate,
        &silence(sample_rate, ONLINE_TAIL_PADDING_SECONDS),
    );
    stream.input_finished();
    decode_ready(recognizer, stream);
    online_text(recognizer, stream)
}

pub fn unload_runtime() {
    if let Ok(mut runtime) = runtime_slot().lock() {
        *runtime = None;
    }
}

/// 返回已加载模型的目录；未加载时为 None
pub fn loaded_model_dir() -> Option<PathBuf> {
    runtime_slot()
        .lock()
        .ok()
        .and_then(|runtime| runtime.as_ref().map(|rt| rt.model_dir.clone()))
}

pub fn is_loaded(models_root: &Path, local_model: &str) -> bool {
    let spec = spec_for_local_model(local_model);
    loaded_model_dir()
        .map(|dir| dir == runtime_model_dir(models_root, spec))
        .unwrap_or(false)
}

pub fn transcribe_wav(
    models_root: &Path,
    local_model: &str,
    language: &str,
    audio_path: &Path,
) -> Result<SherpaResult, SenseVoiceError> {
    let spec = spec_for_local_model(local_model);
    if spec.kind.is_streaming() {
        // 历史重试或流式会话失败时的兜底：把整段录音一次性送入流式识别器
        let recognizer = online_recognizer(models_root, local_model)?;
        let (samples, sample_rate) = read_wav_as_mono_f32(audio_path)?;
        let rate = sample_rate as i32;
        let stream = recognizer.create_stream();
        stream.accept_waveform(rate, &silence(rate, ONLINE_LEAD_SILENCE_SECONDS));
        stream.accept_waveform(rate, &samples);
        decode_ready(&recognizer, &stream);
        let text = finish_online_stream(&recognizer, &stream, rate);
        return Ok(SherpaResult {
            text,
            alignment: None,
        });
    }
    load_runtime(models_root, local_model, language)?;
    let (samples, sample_rate) = read_wav_as_mono_f32(audio_path)?;

    let runtime = runtime_slot()
        .lock()
        .map_err(|_| SenseVoiceError::Process("failed to lock Sherpa runtime".to_string()))?;
    let current = runtime
        .as_ref()
        .ok_or_else(|| SenseVoiceError::Process("Sherpa runtime is not loaded".to_string()))?;
    let recognizer = create_recognizer(&current.model_dir, spec, &current.language)?;

    let stream = recognizer.create_stream();
    stream.accept_waveform(sample_rate as i32, &samples);
    recognizer.decode(&stream);
    let result = stream
        .get_result()
        .ok_or_else(|| SenseVoiceError::Parse("Sherpa recognition result is empty".to_string()))?;
    let alignment = result
        .timestamps
        .as_ref()
        .map(|timestamps| TranscriptionAlignment {
            tokens: result.tokens.clone(),
            timestamps_ms: timestamps
                .iter()
                .map(|value| (*value * 1000.0).max(0.0).round() as u64)
                .collect(),
            durations_ms: result
                .durations
                .unwrap_or_default()
                .iter()
                .map(|value| (*value * 1000.0).max(0.0).round() as u64)
                .collect(),
        });

    Ok(SherpaResult {
        text: result.text,
        alignment,
    })
}

fn create_recognizer(
    model_dir: &Path,
    spec: &LocalModelSpec,
    language: &str,
) -> Result<OfflineRecognizer, SenseVoiceError> {
    let mut config = OfflineRecognizerConfig::default();
    let model_file = |index: usize| -> Option<String> {
        spec.model_files
            .get(index)
            .map(|name| model_dir.join(name).to_string_lossy().to_string())
    };
    let tokens_path = Some(
        model_dir
            .join(TOKENS_FILE_NAME)
            .to_string_lossy()
            .to_string(),
    );
    match spec.kind {
        LocalModelKind::SenseVoice => {
            config.model_config.sense_voice = OfflineSenseVoiceModelConfig {
                model: model_file(0),
                language: Some(language.to_string()),
                use_itn: true,
            };
            config.model_config.tokens = tokens_path;
        }
        LocalModelKind::Paraformer => {
            config.model_config.paraformer = OfflineParaformerModelConfig {
                model: model_file(0),
            };
            config.model_config.tokens = tokens_path;
        }
        LocalModelKind::FireRedAsrCtc => {
            config.model_config.fire_red_asr_ctc = OfflineFireRedAsrCtcModelConfig {
                model: model_file(0),
            };
            config.model_config.tokens = tokens_path;
        }
        LocalModelKind::FireRedAsr => {
            config.model_config.fire_red_asr = OfflineFireRedAsrModelConfig {
                encoder: model_file(0),
                decoder: model_file(1),
            };
            config.model_config.tokens = tokens_path;
        }
        LocalModelKind::FunAsrNano => {
            config.model_config.funasr_nano = OfflineFunASRNanoModelConfig {
                encoder_adaptor: model_file(0),
                llm: model_file(1),
                embedding: model_file(2),
                tokenizer: Some(
                    model_dir
                        .join(FUNASR_NANO_TOKENIZER_DIR)
                        .to_string_lossy()
                        .to_string(),
                ),
                system_prompt: None,
                user_prompt: None,
                max_new_tokens: 512,
                temperature: 1e-6,
                top_p: 0.8,
                seed: 42,
                language: if language.is_empty() {
                    None
                } else {
                    Some(language.to_string())
                },
                itn: 1,
                hotwords: None,
            };
        }
        LocalModelKind::NemoTransducer => {
            config.model_config.transducer = OfflineTransducerModelConfig {
                encoder: model_file(0),
                decoder: model_file(1),
                joiner: model_file(2),
            };
            config.model_config.model_type = Some("nemo_transducer".to_string());
            config.model_config.tokens = tokens_path;
        }
        LocalModelKind::MoonshineV2 => {
            config.model_config.moonshine = OfflineMoonshineModelConfig {
                encoder: model_file(0),
                merged_decoder: model_file(1),
                ..Default::default()
            };
            config.model_config.tokens = tokens_path;
        }
        LocalModelKind::OnlineTransducer => {
            return Err(SenseVoiceError::Config(format!(
                "{} 是流式模型，不能用离线识别器加载",
                spec.display_name
            )));
        }
    }
    config.model_config.num_threads = RECOGNIZER_THREADS;
    OfflineRecognizer::create(&config).ok_or_else(|| {
        SenseVoiceError::Process(format!("failed to create {} recognizer", spec.display_name))
    })
}

fn create_online_recognizer(
    model_dir: &Path,
    spec: &LocalModelSpec,
) -> Result<OnlineRecognizer, SenseVoiceError> {
    let model_file = |index: usize| -> Option<String> {
        spec.model_files
            .get(index)
            .map(|name| model_dir.join(name).to_string_lossy().to_string())
    };
    let mut config = OnlineRecognizerConfig::default();
    config.model_config.transducer = OnlineTransducerModelConfig {
        encoder: model_file(0),
        decoder: model_file(1),
        joiner: model_file(2),
    };
    config.model_config.tokens = Some(
        model_dir
            .join(TOKENS_FILE_NAME)
            .to_string_lossy()
            .to_string(),
    );
    config.model_config.num_threads = RECOGNIZER_THREADS;
    OnlineRecognizer::create(&config).ok_or_else(|| {
        SenseVoiceError::Process(format!("failed to create {} recognizer", spec.display_name))
    })
}

pub struct SherpaResult {
    pub text: String,
    pub alignment: Option<TranscriptionAlignment>,
}

fn normalize_language(spec: &LocalModelSpec, language: &str) -> String {
    let trimmed = language.trim();
    if spec
        .supported_languages
        .iter()
        .any(|lang| *lang == trimmed && trimmed != "auto")
    {
        return trimmed.to_string();
    }
    match spec.kind {
        // SenseVoice 识别 "auto"；Fun-ASR-Nano 使用空值表示自动
        LocalModelKind::FunAsrNano => String::new(),
        _ => "auto".to_string(),
    }
}

fn download_archive<F>(
    archive_path: &Path,
    spec: &LocalModelSpec,
    on_progress: &mut F,
) -> Result<(), SenseVoiceError>
where
    F: FnMut(&str, Option<u8>, Option<u64>, Option<u64>),
{
    let urls = download_urls(spec);
    let mut last_error = String::new();
    for url in &urls {
        match download_archive_from(archive_path, url, on_progress) {
            Ok(()) => return Ok(()),
            Err(err) => {
                let _ = fs::remove_file(archive_path);
                last_error = format!("{url}: {err}");
            }
        }
    }
    Err(SenseVoiceError::Request(format!(
        "模型包下载失败: {last_error}"
    )))
}

fn download_archive_from<F>(
    archive_path: &Path,
    url: &str,
    on_progress: &mut F,
) -> Result<(), SenseVoiceError>
where
    F: FnMut(&str, Option<u8>, Option<u64>, Option<u64>),
{
    let client = Client::builder()
        .connect_timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|err| SenseVoiceError::Request(err.to_string()))?;
    let mut response = client
        .get(url)
        .send()
        .map_err(|err| SenseVoiceError::Request(err.to_string()))?;
    if !response.status().is_success() {
        return Err(SenseVoiceError::Request(format!(
            "failed to download model archive: {}",
            response.status()
        )));
    }

    let total = response.content_length();
    let file = File::create(archive_path).map_err(|err| SenseVoiceError::Io(err.to_string()))?;
    let mut writer = BufWriter::new(file);
    let mut downloaded = 0_u64;
    let mut buf = [0_u8; 64 * 1024];
    loop {
        let read = response
            .read(&mut buf)
            .map_err(|err| SenseVoiceError::Io(err.to_string()))?;
        if read == 0 {
            break;
        }
        writer
            .write_all(&buf[..read])
            .map_err(|err| SenseVoiceError::Io(err.to_string()))?;
        downloaded += read as u64;
        let percent = total.map(|value| ((downloaded as f64 / value as f64) * 90.0).round() as u8);
        if let Some(total) = total {
            on_progress(
                &format!(
                    "Downloaded {} / {} MB",
                    downloaded / 1024 / 1024,
                    total / 1024 / 1024
                ),
                percent,
                Some(downloaded),
                Some(total),
            );
        } else {
            on_progress(
                &format!("Downloaded {} MB", downloaded / 1024 / 1024),
                None,
                Some(downloaded),
                None,
            );
        }
    }
    writer
        .flush()
        .map_err(|err| SenseVoiceError::Io(err.to_string()))?;
    Ok(())
}

fn extract_archive(archive_path: &Path, models_root: &Path) -> Result<(), SenseVoiceError> {
    let archive_file =
        File::open(archive_path).map_err(|err| SenseVoiceError::Io(err.to_string()))?;
    let tar = BzDecoder::new(archive_file);
    let mut archive = Archive::new(tar);
    let entries = archive
        .entries()
        .map_err(|err| SenseVoiceError::Io(err.to_string()))?;

    for entry in entries {
        let mut entry = entry.map_err(|err| SenseVoiceError::Io(err.to_string()))?;
        let unpacked = entry
            .unpack_in(models_root)
            .map_err(|err| SenseVoiceError::Io(err.to_string()))?;
        if !unpacked {
            return Err(SenseVoiceError::Io(
                "refused to extract archive entry outside target directory".to_string(),
            ));
        }
    }

    Ok(())
}

/// 读取 WAV 并下混为单声道；sherpa-onnx 的 accept_waveform 只接受单声道样本，
/// 直接传入交错的多声道数据会被当成双倍时长的音频，导致识别结果完全错误。
fn read_wav_as_mono_f32(audio_path: &Path) -> Result<(Vec<f32>, u32), SenseVoiceError> {
    let mut reader =
        hound::WavReader::open(audio_path).map_err(|err| SenseVoiceError::Io(err.to_string()))?;
    let spec = reader.spec();
    let channels = usize::from(spec.channels.max(1));
    let samples = reader
        .samples::<i16>()
        .map(|sample| {
            sample
                .map(|value| f32::from(value) / f32::from(i16::MAX))
                .map_err(|err| SenseVoiceError::Io(err.to_string()))
        })
        .collect::<Result<Vec<f32>, _>>()?;
    Ok((downmix_to_mono(samples, channels), spec.sample_rate))
}

pub(crate) fn downmix_to_mono(samples: Vec<f32>, channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return samples;
    }
    samples
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::downmix_to_mono;

    /// 用本地已下载的模型跑真实推理（默认忽略）：
    /// VTT_MODELS_ROOT=<模型目录> VTT_TEST_WAV=<16-bit WAV> cargo test --lib -- --ignored --nocapture
    /// 只测目录中已存在的模型。
    #[test]
    #[ignore]
    fn transcribe_with_downloaded_models() {
        use super::{model_is_ready, transcribe_wav};
        use crate::sensevoice::model::LOCAL_MODELS;
        let root = std::path::PathBuf::from(
            std::env::var("VTT_MODELS_ROOT").expect("set VTT_MODELS_ROOT"),
        );
        let wav =
            std::path::PathBuf::from(std::env::var("VTT_TEST_WAV").expect("set VTT_TEST_WAV"));
        let mut tested = 0;
        for spec in LOCAL_MODELS.iter() {
            if !model_is_ready(&root, spec) {
                continue;
            }
            let result = transcribe_wav(&root, spec.model_key, "auto", &wav)
                .unwrap_or_else(|err| panic!("{}: {err}", spec.model_key));
            println!("{}: {}", spec.model_key, result.text);
            assert!(
                !result.text.trim().is_empty(),
                "{} returned empty text",
                spec.model_key
            );
            tested += 1;
        }
        assert!(tested > 0, "no downloaded models found");
    }

    /// 长录音按模型上限切段后逐段转写（默认忽略）：
    /// VTT_MODELS_ROOT=<模型目录> VTT_TEST_LONG_WAV=<长 WAV> VTT_TEST_MODEL=<model key>
    /// cargo test --lib -- --ignored --nocapture
    #[test]
    #[ignore]
    fn transcribe_long_recording_in_segments() {
        use super::transcribe_wav;
        use crate::audio_processing::write_segments;
        use crate::recorder::RecordedAudio;
        use crate::sensevoice::model::spec_for_local_model;
        let root = std::path::PathBuf::from(
            std::env::var("VTT_MODELS_ROOT").expect("set VTT_MODELS_ROOT"),
        );
        let wav = std::env::var("VTT_TEST_LONG_WAV").expect("set VTT_TEST_LONG_WAV");
        let model = std::env::var("VTT_TEST_MODEL").expect("set VTT_TEST_MODEL");
        let mut reader = hound::WavReader::open(&wav).expect("open wav");
        let spec = reader.spec();
        let samples: Vec<i16> = reader.samples::<i16>().map(|s| s.unwrap()).collect();
        let audio = RecordedAudio {
            samples,
            sample_rate: spec.sample_rate,
            channels: spec.channels,
        };
        let limit = spec_for_local_model(&model)
            .kind
            .max_chunk_seconds()
            .unwrap_or(60);
        let segments = write_segments(&audio, limit).expect("write segments");
        let mut words = Vec::new();
        for segment in &segments {
            let text = transcribe_wav(&root, &model, "auto", &segment.path)
                .expect("transcribe")
                .text;
            println!("[{}ms] {}", segment.start_ms, text);
            words.extend(text.split_whitespace().map(str::to_lowercase));
        }
        // 重复输出（hallucination）时同一个词会连续出现很多次
        let longest_run = words
            .chunk_by(|a, b| a == b)
            .map(|run| run.len())
            .max()
            .unwrap_or(0);
        assert!(!words.is_empty());
        assert!(longest_run < 4, "repeated word run of {longest_run}");
    }

    #[test]
    fn downmix_averages_interleaved_frames() {
        assert_eq!(
            downmix_to_mono(vec![0.2, 0.4, -1.0, 1.0], 2),
            vec![0.3, 0.0]
        );
        assert_eq!(downmix_to_mono(vec![0.5, -0.5], 1), vec![0.5, -0.5]);
    }
}
