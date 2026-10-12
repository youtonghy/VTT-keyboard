use super::{model::spec_for_local_model, sherpa, SenseVoiceError};
use crate::settings::TranscriptionAlignment;
use std::fs;
use std::path::{Path, PathBuf};

pub fn set_models_root(models_root: PathBuf) {
    sherpa::set_models_root(models_root);
}

pub fn get_models_root() -> Option<PathBuf> {
    sherpa::get_models_root()
}

/// 指定模型的文件是否已完整下载
pub fn model_files_ready(local_model: &str) -> bool {
    let Some(root) = get_models_root() else {
        return false;
    };
    sherpa::model_is_ready(&root, spec_for_local_model(local_model))
}

pub fn prepare_model<F>(
    local_model: &str,
    models_root: &Path,
    on_progress: F,
) -> Result<(), SenseVoiceError>
where
    F: FnMut(&str, Option<u8>, Option<u64>, Option<u64>),
{
    sherpa::prepare_model(models_root, local_model, on_progress)
}

pub fn load(local_model: &str, models_root: &Path, language: &str) -> Result<(), SenseVoiceError> {
    sherpa::load_runtime(models_root, local_model, language)
}

pub fn unload(_local_model: &str) {
    sherpa::unload_runtime();
}

/// 卸载并删除指定模型的全部文件（模型目录与残留的下载包）
pub fn remove_model_files(local_model: &str) -> Result<(), SenseVoiceError> {
    sherpa::unload_runtime();
    let Some(root) = get_models_root() else {
        return Ok(());
    };
    let spec = spec_for_local_model(local_model);
    let model_dir = sherpa::runtime_model_dir(&root, spec);
    if model_dir.exists() {
        fs::remove_dir_all(&model_dir)
            .map_err(|err| SenseVoiceError::Io(format!("删除模型目录失败: {err}")))?;
    }
    let archive_path = root.join(spec.archive_name);
    if archive_path.exists() {
        fs::remove_file(&archive_path)
            .map_err(|err| SenseVoiceError::Io(format!("删除模型包失败: {err}")))?;
    }
    Ok(())
}

pub fn transcribe_wav(
    local_model: &str,
    language: &str,
    audio_path: &Path,
) -> Result<NativeRuntimeTranscription, SenseVoiceError> {
    let models_root = get_models_root()
        .ok_or_else(|| SenseVoiceError::Config("本地模型目录尚未初始化".to_string()))?;
    let result = sherpa::transcribe_wav(&models_root, local_model, language, audio_path)?;
    Ok(NativeRuntimeTranscription {
        text: result.text,
        alignment: result.alignment,
    })
}

pub struct NativeRuntimeTranscription {
    pub text: String,
    pub alignment: Option<TranscriptionAlignment>,
}
