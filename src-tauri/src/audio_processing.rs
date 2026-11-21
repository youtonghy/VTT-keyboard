use crate::recorder::RecordedAudio;
use hound::{SampleFormat, WavSpec, WavWriter};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// 在每段上限之前的这段时间内寻找最安静的位置作为切点
const SPLIT_SEARCH_SECONDS: f64 = 3.0;
/// 计算能量的窗口长度
const ENERGY_WINDOW_SECONDS: f64 = 0.02;

#[derive(Debug, Error)]
pub enum AudioProcessingError {
    #[error("无法写入录音文件: {0}")]
    Io(String),
}

/// 写入磁盘的一段录音
pub struct AudioSegment {
    pub path: PathBuf,
    /// 该段在整段录音中的起始时间
    pub start_ms: u64,
}

/// 把录音切成不超过 `max_segment_seconds` 的若干段并写成 WAV。
/// 超长时在上限前最安静的位置切开，避免把一个字切成两半。
pub fn write_segments(
    audio: &RecordedAudio,
    max_segment_seconds: u64,
) -> Result<Vec<AudioSegment>, AudioProcessingError> {
    let dir = std::env::temp_dir().join("vtt-keyboard").join("recordings");
    fs::create_dir_all(&dir).map_err(|err| AudioProcessingError::Io(err.to_string()))?;

    let channels = usize::from(audio.channels.max(1));
    let ranges = split_frames(
        &audio.samples,
        channels,
        audio.sample_rate,
        max_segment_seconds,
    );
    let mut segments = Vec::with_capacity(ranges.len());
    for (index, (start, end)) in ranges.into_iter().enumerate() {
        let path = dir.join(format!("segment-{index}.wav"));
        write_wav(
            &path,
            audio,
            &audio.samples[start * channels..end * channels],
        )?;
        segments.push(AudioSegment {
            path,
            start_ms: frames_to_ms(start, audio.sample_rate),
        });
    }
    Ok(segments)
}

fn frames_to_ms(frames: usize, sample_rate: u32) -> u64 {
    if sample_rate == 0 {
        return 0;
    }
    (frames as u64 * 1000) / u64::from(sample_rate)
}

/// 计算分段的帧区间 `[start, end)`（一帧 = 每个声道各一个样本）。
fn split_frames(
    samples: &[i16],
    channels: usize,
    sample_rate: u32,
    max_segment_seconds: u64,
) -> Vec<(usize, usize)> {
    let channels = channels.max(1);
    let total_frames = samples.len() / channels;
    let rate = sample_rate.max(1) as usize;
    let max_frames = (rate * max_segment_seconds.max(1) as usize).max(1);
    // 搜索区间不超过半段，保证每段至少前进半个上限
    let search_frames = ((rate as f64 * SPLIT_SEARCH_SECONDS) as usize).min(max_frames / 2);
    let window_frames = ((rate as f64 * ENERGY_WINDOW_SECONDS) as usize).max(1);

    let mut ranges = Vec::new();
    let mut start = 0;
    while start < total_frames {
        let limit = start + max_frames;
        if limit >= total_frames {
            ranges.push((start, total_frames));
            break;
        }
        let end = quietest_cut(
            samples,
            channels,
            limit - search_frames,
            limit,
            window_frames,
        );
        ranges.push((start, end));
        start = end;
    }
    ranges
}

/// 在 `[from, to)` 帧范围内找能量最低的窗口，返回该窗口中点作为切点。
fn quietest_cut(
    samples: &[i16],
    channels: usize,
    from: usize,
    to: usize,
    window_frames: usize,
) -> usize {
    if to <= from + window_frames {
        return to;
    }
    let mut best_cut = to;
    let mut best_energy = u64::MAX;
    let mut window_start = from;
    while window_start + window_frames <= to {
        let energy: u64 = samples
            [window_start * channels..(window_start + window_frames) * channels]
            .iter()
            .map(|sample| {
                let value = i64::from(*sample);
                (value * value) as u64
            })
            .sum();
        // 能量相同时取更靠后的位置，让每段尽量长
        if energy <= best_energy {
            best_energy = energy;
            best_cut = window_start + window_frames / 2;
        }
        window_start += window_frames;
    }
    best_cut
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
    let mut writer =
        WavWriter::create(path, spec).map_err(|err| AudioProcessingError::Io(err.to_string()))?;
    for sample in samples {
        writer
            .write_sample(*sample)
            .map_err(|err| AudioProcessingError::Io(err.to_string()))?;
    }
    writer
        .finalize()
        .map_err(|err| AudioProcessingError::Io(err.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 1000;

    /// 生成 `seconds` 秒的单声道“说话声”（满幅方波）
    fn loud(seconds: usize) -> Vec<i16> {
        (0..seconds * RATE as usize)
            .map(|i| if i % 2 == 0 { 8000 } else { -8000 })
            .collect()
    }

    fn assert_contiguous(ranges: &[(usize, usize)], total: usize, max_frames: usize) {
        assert_eq!(ranges.first().map(|r| r.0), Some(0));
        assert_eq!(ranges.last().map(|r| r.1), Some(total));
        for pair in ranges.windows(2) {
            assert_eq!(pair[0].1, pair[1].0);
        }
        for (start, end) in ranges {
            assert!(end > start);
            assert!(end - start <= max_frames, "{start}..{end}");
        }
    }

    #[test]
    fn short_audio_is_a_single_segment() {
        let samples = loud(5);
        assert_eq!(split_frames(&samples, 1, RATE, 10), vec![(0, 5000)]);
    }

    #[test]
    fn empty_audio_has_no_segments() {
        assert!(split_frames(&[], 1, RATE, 10).is_empty());
    }

    #[test]
    fn cuts_inside_the_pause_before_the_limit() {
        // 8.5 秒说话 + 0.5 秒停顿 + 6 秒说话，上限 10 秒
        let mut samples = loud(8);
        samples.extend(vec![8000_i16; 500]);
        samples.extend(vec![0_i16; 500]);
        samples.extend(loud(6));
        let ranges = split_frames(&samples, 1, RATE, 10);
        assert_contiguous(&ranges, samples.len(), 10_000);
        let cut = ranges[0].1;
        assert!((8500..9000).contains(&cut), "cut at {cut}");
    }

    #[test]
    fn continuous_speech_still_respects_the_limit() {
        let samples = loud(35);
        let ranges = split_frames(&samples, 1, RATE, 10);
        assert_contiguous(&ranges, samples.len(), 10_000);
        assert_eq!(ranges.len(), 4);
    }

    #[test]
    fn stereo_ranges_are_in_frames() {
        let mono = loud(25);
        let stereo: Vec<i16> = mono.iter().flat_map(|s| [*s, *s]).collect();
        let ranges = split_frames(&stereo, 2, RATE, 10);
        assert_contiguous(&ranges, mono.len(), 10_000);
    }

    #[test]
    fn frames_convert_to_milliseconds() {
        assert_eq!(frames_to_ms(48_000, 48_000), 1000);
        assert_eq!(frames_to_ms(8_000, 16_000), 500);
        assert_eq!(frames_to_ms(10, 0), 0);
    }
}
