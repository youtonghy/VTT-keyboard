use super::SenseVoiceError;
use crate::settings::TranscriptionAlignment;
use bzip2::read::BzDecoder;
use reqwest::blocking::Client;
use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig, OfflineSenseVoiceModelConfig};
use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use tar::Archive;

const MODEL_ARCHIVE_NAME: &str = "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09.tar.bz2";
const MODEL_DOWNLOAD_URL: &str =
    "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09.tar.bz2";
const MODEL_DIR_NAME: &str = "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2025-09-09";
const MODEL_FILE_NAME: &str = "model.int8.onnx";
const TOKENS_FILE_NAME: &str = "tokens.txt";

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
    models_root_slot()
        .lock()
        .ok()
        .and_then(|slot| slot.clone())
}

pub fn runtime_model_dir(models_root: &Path) -> PathBuf {
    models_root.join(MODEL_DIR_NAME)
}

pub fn model_is_ready(models_root: &Path) -> bool {
    let model_dir = runtime_model_dir(models_root);
    model_dir.join(MODEL_FILE_NAME).exists() && model_dir.join(TOKENS_FILE_NAME).exists()
}

pub fn prepare_model<F>(models_root: &Path, mut on_progress: F) -> Result<(), SenseVoiceError>
where
    F: FnMut(&str, Option<u8>, Option<u64>, Option<u64>),
{
    if model_is_ready(models_root) {
        return Ok(());
    }

    fs::create_dir_all(models_root).map_err(|err| SenseVoiceError::Io(err.to_string()))?;

    let archive_path = models_root.join(MODEL_ARCHIVE_NAME);
    on_progress(&format!("Downloading {MODEL_ARCHIVE_NAME}"), Some(0), Some(0), None);
    download_archive(&archive_path, &mut on_progress)?;
    on_progress("Extracting Sherpa-ONNX SenseVoice model", Some(95), None, None);
    extract_archive(&archive_path, models_root)?;
    let _ = fs::remove_file(&archive_path);

    if !model_is_ready(models_root) {
        return Err(SenseVoiceError::Io(
            "Sherpa-ONNX model files are incomplete after extraction".to_string(),
        ));
    }
    Ok(())
}

pub fn load_runtime(models_root: &Path, language: &str) -> Result<(), SenseVoiceError> {
    let model_dir = runtime_model_dir(models_root);
    if !model_is_ready(models_root) {
        return Err(SenseVoiceError::Config(
            "Sherpa-ONNX SenseVoice model is not downloaded yet".to_string(),
        ));
    }

    let normalized_language = normalize_language(language);
    let mut runtime = runtime_slot()
        .lock()
        .map_err(|_| SenseVoiceError::Process("failed to lock Sherpa runtime".to_string()))?;
    let should_reload = runtime
        .as_ref()
        .is_none_or(|current| current.model_dir != model_dir || current.language != normalized_language);
    if !should_reload {
        return Ok(());
    }

    let _ = create_recognizer(&model_dir, &normalized_language)?;

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

pub fn runtime_is_loaded() -> bool {
    runtime_slot()
        .lock()
        .ok()
        .and_then(|runtime| runtime.as_ref().map(|_| true))
        .unwrap_or(false)
}

pub fn transcribe_wav(models_root: &Path, language: &str, audio_path: &Path) -> Result<SherpaResult, SenseVoiceError> {
    load_runtime(models_root, language)?;
    let samples = read_wav_as_f32(audio_path)?;
    let sample_rate = read_sample_rate(audio_path)?;

    let runtime = runtime_slot()
        .lock()
        .map_err(|_| SenseVoiceError::Process("failed to lock Sherpa runtime".to_string()))?;
    let current = runtime
        .as_ref()
        .ok_or_else(|| SenseVoiceError::Process("Sherpa runtime is not loaded".to_string()))?;
    let recognizer = create_recognizer(&current.model_dir, &current.language)?;

    let stream = recognizer.create_stream();
    stream.accept_waveform(sample_rate as i32, &samples);
    recognizer.decode(&stream);
    let result = stream
        .get_result()
        .ok_or_else(|| SenseVoiceError::Parse("Sherpa recognition result is empty".to_string()))?;
    let alignment = result.timestamps.as_ref().map(|timestamps| TranscriptionAlignment {
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

fn create_recognizer(model_dir: &Path, language: &str) -> Result<OfflineRecognizer, SenseVoiceError> {
    let mut config = OfflineRecognizerConfig::default();
    config.model_config.sense_voice = OfflineSenseVoiceModelConfig {
        model: Some(model_dir.join(MODEL_FILE_NAME).to_string_lossy().to_string()),
        language: Some(language.to_string()),
        use_itn: true,
    };
    config.model_config.tokens = Some(model_dir.join(TOKENS_FILE_NAME).to_string_lossy().to_string());
    config.model_config.num_threads = 2;
    OfflineRecognizer::create(&config).ok_or_else(|| {
        SenseVoiceError::Process("failed to create Sherpa-ONNX SenseVoice recognizer".to_string())
    })
}

pub struct SherpaResult {
    pub text: String,
    pub alignment: Option<TranscriptionAlignment>,
}

fn normalize_language(language: &str) -> String {
    match language.trim() {
        "zh" | "en" | "ja" | "ko" | "yue" => language.trim().to_string(),
        _ => "auto".to_string(),
    }
}

fn download_archive<F>(archive_path: &Path, on_progress: &mut F) -> Result<(), SenseVoiceError>
where
    F: FnMut(&str, Option<u8>, Option<u64>, Option<u64>),
{
    let client = Client::new();
    let mut response = client
        .get(MODEL_DOWNLOAD_URL)
        .send()
        .map_err(|err| SenseVoiceError::Request(err.to_string()))?;
    if !response.status().is_success() {
        return Err(SenseVoiceError::Request(format!(
            "failed to download Sherpa model archive: {}",
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
                &format!("Downloaded {} / {} MB", downloaded / 1024 / 1024, total / 1024 / 1024),
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
    writer.flush().map_err(|err| SenseVoiceError::Io(err.to_string()))?;
    Ok(())
}

fn extract_archive(archive_path: &Path, models_root: &Path) -> Result<(), SenseVoiceError> {
    let archive_file = File::open(archive_path).map_err(|err| SenseVoiceError::Io(err.to_string()))?;
    let tar = BzDecoder::new(archive_file);
    let mut archive = Archive::new(tar);
    archive
        .unpack(models_root)
        .map_err(|err| SenseVoiceError::Io(err.to_string()))
}

fn read_wav_as_f32(audio_path: &Path) -> Result<Vec<f32>, SenseVoiceError> {
    let mut reader = hound::WavReader::open(audio_path).map_err(|err| SenseVoiceError::Io(err.to_string()))?;
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
    let reader = hound::WavReader::open(audio_path).map_err(|err| SenseVoiceError::Io(err.to_string()))?;
    Ok(reader.spec().sample_rate)
}
