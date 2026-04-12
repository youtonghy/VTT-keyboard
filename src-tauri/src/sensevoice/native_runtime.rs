use super::{model::LOCAL_MODEL_SHERPA_ONNX_SENSEVOICE, sherpa, SenseVoiceError};
use crate::settings::TranscriptionAlignment;
use std::path::{Path, PathBuf};

pub fn supports_local_model(local_model: &str) -> bool {
    local_model.eq_ignore_ascii_case(LOCAL_MODEL_SHERPA_ONNX_SENSEVOICE)
}

pub fn set_models_root(models_root: PathBuf) {
    sherpa::set_models_root(models_root);
}

pub fn get_models_root() -> Option<PathBuf> {
    sherpa::get_models_root()
}

pub fn is_loaded(local_model: &str) -> bool {
    supports_local_model(local_model) && sherpa::runtime_is_loaded()
}

pub fn prepare_model<F>(
    local_model: &str,
    models_root: &Path,
    on_progress: F,
) -> Result<(), SenseVoiceError>
where
    F: FnMut(&str, Option<u8>, Option<u64>, Option<u64>),
{
    if supports_local_model(local_model) {
        return sherpa::prepare_model(models_root, on_progress);
    }
    Err(SenseVoiceError::Config(format!(
        "unsupported native local model: {local_model}"
    )))
}

pub fn load(local_model: &str, models_root: &Path, language: &str) -> Result<(), SenseVoiceError> {
    if supports_local_model(local_model) {
        return sherpa::load_runtime(models_root, language);
    }
    Err(SenseVoiceError::Config(format!(
        "unsupported native local model: {local_model}"
    )))
}

pub fn unload(local_model: &str) {
    if supports_local_model(local_model) {
        sherpa::unload_runtime();
    }
}

pub fn transcribe_wav(
    local_model: &str,
    language: &str,
    audio_path: &Path,
) -> Result<NativeRuntimeTranscription, SenseVoiceError> {
    let models_root = get_models_root().ok_or_else(|| {
        SenseVoiceError::Config("Native model directory is not initialized".to_string())
    })?;
    if supports_local_model(local_model) {
        let result = sherpa::transcribe_wav(&models_root, language, audio_path)?;
        return Ok(NativeRuntimeTranscription {
            text: result.text,
            alignment: result.alignment,
        });
    }
    Err(SenseVoiceError::Config(format!(
        "unsupported native local model: {local_model}"
    )))
}

pub struct NativeRuntimeTranscription {
    pub text: String,
    pub alignment: Option<TranscriptionAlignment>,
}
