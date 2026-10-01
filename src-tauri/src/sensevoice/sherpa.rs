use super::model::{download_urls, spec_for_local_model, LocalModelKind, LocalModelSpec};
use crate::sensevoice::SenseVoiceError;
use crate::settings::TranscriptionAlignment;
use bzip2::read::BzDecoder;
use reqwest::blocking::Client;
use sherpa_onnx::{
    OfflineFireRedAsrCtcModelConfig, OfflineFunASRNanoModelConfig, OfflineParaformerModelConfig,
    OfflineRecognizer, OfflineRecognizerConfig, OfflineSenseVoiceModelConfig,
};
use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use tar::Archive;

const TOKENS_FILE_NAME: &str = "tokens.txt";
const FUNASR_NANO_TOKENIZER_DIR: &str = "Qwen3-0.6B";

struct SherpaRuntime {
    model_dir: PathBuf,
    language: String,
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
        return Err(SenseVoiceError::Io(
            "模型文件解压后不完整".to_string(),
        ));
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

    let _ = create_recognizer(&model_dir, spec, &normalized_language)?;

    *runtime = Some(SherpaRuntime {
        model_dir,
        language: normalized_language,
    });
    Ok(())
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
    load_runtime(models_root, local_model, language)?;
    let spec = spec_for_local_model(local_model);
    let samples = read_wav_as_f32(audio_path)?;
    let sample_rate = read_sample_rate(audio_path)?;

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
    let result = stream.get_result().ok_or_else(|| {
        SenseVoiceError::Parse("Sherpa recognition result is empty".to_string())
    })?;
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
    let model_path = model_dir
        .join("model.int8.onnx")
        .to_string_lossy()
        .to_string();
    match spec.kind {
        LocalModelKind::SenseVoice => {
            config.model_config.sense_voice = OfflineSenseVoiceModelConfig {
                model: Some(model_path),
                language: Some(language.to_string()),
                use_itn: true,
            };
            config.model_config.tokens = Some(
                model_dir
                    .join(TOKENS_FILE_NAME)
                    .to_string_lossy()
                    .to_string(),
            );
        }
        LocalModelKind::Paraformer => {
            config.model_config.paraformer = OfflineParaformerModelConfig {
                model: Some(model_path),
            };
            config.model_config.tokens = Some(
                model_dir
                    .join(TOKENS_FILE_NAME)
                    .to_string_lossy()
                    .to_string(),
            );
        }
        LocalModelKind::FireRedAsrCtc => {
            config.model_config.fire_red_asr_ctc = OfflineFireRedAsrCtcModelConfig {
                model: Some(model_path),
            };
            config.model_config.tokens = Some(
                model_dir
                    .join(TOKENS_FILE_NAME)
                    .to_string_lossy()
                    .to_string(),
            );
        }
        LocalModelKind::FunAsrNano => {
            config.model_config.funasr_nano = OfflineFunASRNanoModelConfig {
                encoder_adaptor: Some(
                    model_dir
                        .join("encoder_adaptor.int8.onnx")
                        .to_string_lossy()
                        .to_string(),
                ),
                llm: Some(
                    model_dir
                        .join("llm.int8.onnx")
                        .to_string_lossy()
                        .to_string(),
                ),
                embedding: Some(
                    model_dir
                        .join("embedding.int8.onnx")
                        .to_string_lossy()
                        .to_string(),
                ),
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
    }
    config.model_config.num_threads = 2;
    OfflineRecognizer::create(&config).ok_or_else(|| {
        SenseVoiceError::Process(format!(
            "failed to create {} recognizer",
            spec.display_name
        ))
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

fn read_wav_as_f32(audio_path: &Path) -> Result<Vec<f32>, SenseVoiceError> {
    let mut reader =
        hound::WavReader::open(audio_path).map_err(|err| SenseVoiceError::Io(err.to_string()))?;
    reader
        .samples::<i16>()
        .map(|sample| {
            sample
                .map(|value| f32::from(value) / f32::from(i16::MAX))
                .map_err(|err| SenseVoiceError::Io(err.to_string()))
        })
        .collect()
}

fn read_sample_rate(audio_path: &Path) -> Result<u32, SenseVoiceError> {
    let reader =
        hound::WavReader::open(audio_path).map_err(|err| SenseVoiceError::Io(err.to_string()))?;
    Ok(reader.spec().sample_rate)
}
