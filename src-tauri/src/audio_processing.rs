use crate::recorder::RecordedAudio;
use hound::{SampleFormat, WavSpec, WavWriter};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tauri::{AppHandle, Manager};

#[derive(Debug, Error)]
pub enum AudioProcessingError {
    #[error("无法获取缓存目录: {0}")]
    Path(String),
    #[error("无法写入录音文件: {0}")]
    Io(String),
}

pub fn write_segments(
    app: &AppHandle,
    audio: &RecordedAudio,
    segment_seconds: u64,
) -> Result<Vec<PathBuf>, AudioProcessingError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|err| AudioProcessingError::Path(err.to_string()))?
        .join("recordings");
    fs::create_dir_all(&dir).map_err(|err| AudioProcessingError::Io(err.to_string()))?;

    let total_samples = audio.samples.len();
    let samples_per_second = audio.sample_rate as usize * audio.channels as usize;
    let segment_samples = samples_per_second * segment_seconds as usize;

    let mut paths = Vec::new();
    let mut offset = 0;
    let mut index = 0;
    while offset < total_samples {
        let end = (offset + segment_samples).min(total_samples);
        let path = dir.join(format!("segment-{index}.wav"));
        write_wav(&path, audio, &audio.samples[offset..end])?;
        paths.push(path);
        offset = end;
        index += 1;
    }

    Ok(paths)
}

fn write_wav(
    path: &Path,
    audio: &RecordedAudio,
    samples: &[i16],
) -> Result<(), AudioProcessingError> {
    let spec = WavSpec {
        channels: audio.channels,
        sample_rate: audio.sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut writer = WavWriter::create(path, spec)
        .map_err(|err| AudioProcessingError::Io(err.to_string()))?;
    for sample in samples {
        writer
            .write_sample(*sample)
            .map_err(|err| AudioProcessingError::Io(err.to_string()))?;
    }
    writer.finalize().map_err(|err| AudioProcessingError::Io(err.to_string()))?;
    Ok(())
}
