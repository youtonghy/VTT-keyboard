use crate::audio_processing;
use crate::paste;
use crate::recorder::RecordedAudio;
use crate::settings::{SettingsStore, TranscriptionAlignment, TriggerMatch};
use crate::status_native::{self, StatusType};
use crate::transcription;
use crate::triggers;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

/// Counter to track status show operations, used to prevent race conditions
/// when hiding the status window after a delay.
static STATUS_COUNTER: AtomicU64 = AtomicU64::new(0);

fn dev_log(_message: &str) {
    #[cfg(debug_assertions)]
    {
        eprintln!("[DEV] {}", _message);
    }
}

#[derive(Clone)]
pub struct ProcessingOutcome {
    pub history_enabled: bool,
    pub transcription_text: String,
    pub final_text: String,
    pub model_group: String,
    pub transcription_elapsed_ms: u64,
    pub recording_duration_ms: u64,
    pub triggered: bool,
    pub triggered_by_keyword: bool,
    pub trigger_matches: Vec<TriggerMatch>,
    pub alignment: Option<TranscriptionAlignment>,
    pub error_message: Option<String>,
}

impl ProcessingOutcome {
    fn builder() -> ProcessingOutcomeBuilder {
        ProcessingOutcomeBuilder::default()
    }

    pub fn is_success(&self) -> bool {
        self.error_message.is_none()
    }
}

#[derive(Default)]
struct ProcessingOutcomeBuilder {
    history_enabled: bool,
    transcription_text: String,
    final_text: String,
    model_group: String,
    transcription_elapsed_ms: u64,
    recording_duration_ms: u64,
    triggered: bool,
    triggered_by_keyword: bool,
    trigger_matches: Vec<TriggerMatch>,
    alignment: Option<TranscriptionAlignment>,
}

impl ProcessingOutcomeBuilder {
    fn history_enabled(mut self, v: bool) -> Self { self.history_enabled = v; self }
    fn transcription_text(mut self, v: String) -> Self { self.transcription_text = v; self }
    fn final_text(mut self, v: String) -> Self { self.final_text = v; self }
    fn model_group(mut self, v: String) -> Self { self.model_group = v; self }
    fn transcription_elapsed_ms(mut self, v: u64) -> Self { self.transcription_elapsed_ms = v; self }
    fn recording_duration_ms(mut self, v: u64) -> Self { self.recording_duration_ms = v; self }
    fn triggered(mut self, v: bool) -> Self { self.triggered = v; self }
    fn triggered_by_keyword(mut self, v: bool) -> Self { self.triggered_by_keyword = v; self }
    fn trigger_matches(mut self, v: Vec<TriggerMatch>) -> Self { self.trigger_matches = v; self }
    fn alignment(mut self, v: Option<TranscriptionAlignment>) -> Self { self.alignment = v; self }

    fn build(self) -> ProcessingOutcome {
        ProcessingOutcome {
            history_enabled: self.history_enabled,
            transcription_text: self.transcription_text,
            final_text: self.final_text,
            model_group: self.model_group,
            transcription_elapsed_ms: self.transcription_elapsed_ms,
            recording_duration_ms: self.recording_duration_ms,
            triggered: self.triggered,
            triggered_by_keyword: self.triggered_by_keyword,
            trigger_matches: self.trigger_matches,
            alignment: self.alignment,
            error_message: None,
        }
    }

    fn build_error(self, msg: impl Into<String>) -> ProcessingOutcome {
        ProcessingOutcome {
            history_enabled: self.history_enabled,
            transcription_text: self.transcription_text,
            final_text: self.final_text,
            model_group: self.model_group,
            transcription_elapsed_ms: self.transcription_elapsed_ms,
            recording_duration_ms: self.recording_duration_ms,
            triggered: self.triggered,
            triggered_by_keyword: self.triggered_by_keyword,
            trigger_matches: self.trigger_matches,
            alignment: self.alignment,
            error_message: Some(msg.into()),
        }
    }
}

pub fn handle_recording(store: &SettingsStore, recording: RecordedAudio) -> ProcessingOutcome {
    let settings = match store.load() {
        Ok(value) => value,
        Err(err) => {
            return ProcessingOutcome::builder()
                .build_error(format!("设置读取失败: {err}"));
        }
    };
    let history_enabled = settings.history.enabled;
    let remove_newlines = settings.output.remove_newlines;
    let engine = transcription::create_engine(&settings);
    let model_group = engine.model_group();
    let recording_duration_ms = calculate_recording_duration_ms(&recording);

    // Common builder with shared context
    let base = || {
        ProcessingOutcome::builder()
            .history_enabled(history_enabled)
            .model_group(model_group.clone())
            .recording_duration_ms(recording_duration_ms)
    };

    if recording.samples.is_empty() {
        dev_log("录音为空，跳过转写");
        emit_status("completed");
        return base().build();
    }
    let transcription_started = Instant::now();
    let segment_seconds = settings.recording.segment_seconds.max(1);
    dev_log(&format!(
        "开始转写，采样 {}，分段秒数 {}",
        recording.samples.len(),
        segment_seconds
    ));
    let paths = match audio_processing::write_segments(&recording, segment_seconds) {
        Ok(value) => value,
        Err(err) => {
            return base()
                .transcription_elapsed_ms(elapsed_since_ms(transcription_started))
                .build_error(format!("录音分段失败: {err}"));
        }
    };
    dev_log(&format!("生成 {} 段录音", paths.len()));

    let mut transcripts = Vec::new();
    let mut alignment_tokens = Vec::new();
    let mut alignment_timestamps_ms = Vec::new();
    let mut alignment_durations_ms = Vec::new();
    for (index, path) in paths.iter().enumerate() {
        dev_log(&format!("开始请求转写段落 {}", index + 1));
        let transcription = match engine.transcribe(path) {
            Ok(value) => value,
            Err(err) => {
                cleanup_files(&paths);
                let partial = normalize_text_for_output(&transcripts.join(" "), remove_newlines);
                return base()
                    .transcription_text(partial.clone())
                    .final_text(partial)
                    .transcription_elapsed_ms(elapsed_since_ms(transcription_started))
                    .build_error(err.to_string());
            }
        };
        let text = transcription.text;
        if let Some(alignment) = transcription.alignment {
            let segment_offset_ms = index as u64 * segment_seconds * 1000;
            alignment_tokens.extend(alignment.tokens);
            alignment_timestamps_ms.extend(
                alignment
                    .timestamps_ms
                    .into_iter()
                    .map(|timestamp_ms| timestamp_ms + segment_offset_ms),
            );
            alignment_durations_ms.extend(alignment.durations_ms);
        }
        dev_log(&format!("转写结果 {}: {}", index + 1, text));
        transcripts.push(text);
    }

    cleanup_files(&paths);

    let combined = normalize_text_for_output(&transcripts.join(" "), remove_newlines);
    let alignment = if alignment_tokens.is_empty() {
        None
    } else {
        Some(TranscriptionAlignment {
            tokens: alignment_tokens,
            timestamps_ms: alignment_timestamps_ms,
            durations_ms: alignment_durations_ms,
        })
    };
    let transcription_elapsed_ms = elapsed_since_ms(transcription_started);
    dev_log(&format!("合并转写结果: {}", combined));

    // After transcription, build the post-transcription base
    let post = || {
        base()
            .transcription_text(combined.clone())
            .transcription_elapsed_ms(transcription_elapsed_ms)
            .alignment(alignment.clone())
    };

    let logger = |message: &str| dev_log(message);
    let result = match triggers::apply_triggers(&settings, &combined, &logger) {
        Ok(value) => value,
        Err(err) => {
            return post()
                .final_text(combined.clone())
                .build_error(format!("触发词处理失败: {err}"));
        }
    };
    dev_log(&format!(
        "触发词处理: {}",
        if result.triggered {
            "已触发"
        } else {
            "未触发"
        }
    ));
    dev_log(&format!("触发词输出: {}", result.output));
    let final_output = normalize_text_for_output(&result.output, remove_newlines);

    let post_trigger = || {
        post()
            .final_text(final_output.clone())
            .triggered(result.triggered)
            .triggered_by_keyword(result.triggered_by_keyword)
            .trigger_matches(result.trigger_matches.clone())
    };

    if result.triggered {
        dev_log("复制原文到剪贴板");
        if let Err(err) = paste::write_text(&combined) {
            return post_trigger()
                .build_error(format!("写入剪贴板失败: {err}"));
        }
    }
    dev_log("写入并粘贴处理后的文本");
    if let Err(err) = paste::write_and_paste(&final_output) {
        return post_trigger()
            .build_error(format!("写入剪贴板失败: {err}"));
    }
    emit_status("completed");
    post_trigger().build()
}

/// Show status overlay with native window.
/// For "completed" and "error" status, auto-hide after 2 seconds.
pub fn emit_status(status: &str) {
    let (status_type, text) = match status {
        "recording" => (StatusType::Recording, "正在录音"),
        "transcribing" => (StatusType::Transcribing, "正在转写"),
        "completed" => (StatusType::Completed, "已完成"),
        "error" => (StatusType::Error, "已中断"),
        _ => return,
    };

    // Increment counter to invalidate any pending hide operations
    let current_count = STATUS_COUNTER.fetch_add(1, Ordering::SeqCst) + 1;

    status_native::show(status_type, text);

    // Auto-hide after 2 seconds for completed/error states
    // Only hide if no new status was shown during the delay
    if status_type == StatusType::Completed || status_type == StatusType::Error {
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(2));
            // Only hide if the counter hasn't changed (no new status was shown)
            if STATUS_COUNTER.load(Ordering::SeqCst) == current_count {
                status_native::hide();
            }
        });
    }
}

fn cleanup_files(paths: &[std::path::PathBuf]) {
    for path in paths {
        let _ = fs::remove_file(path);
    }
}

fn normalize_text_for_output(text: &str, remove_newlines: bool) -> String {
    if remove_newlines {
        remove_line_breaks(text)
    } else {
        text.to_string()
    }
}

fn remove_line_breaks(text: &str) -> String {
    text.chars()
        .filter(|ch| *ch != '\n' && *ch != '\r')
        .collect()
}

fn calculate_recording_duration_ms(recording: &RecordedAudio) -> u64 {
    let samples_per_second =
        u64::from(recording.sample_rate).saturating_mul(u64::from(recording.channels));
    if samples_per_second == 0 {
        return 0;
    }
    let sample_count = recording.samples.len() as u128;
    let duration_ms = sample_count.saturating_mul(1000) / u128::from(samples_per_second);
    duration_ms as u64
}

fn elapsed_since_ms(started: Instant) -> u64 {
    started.elapsed().as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::{calculate_recording_duration_ms, remove_line_breaks};
    use crate::recorder::RecordedAudio;

    #[test]
    fn remove_line_breaks_removes_crlf_lf_and_cr() {
        let input = "a\r\nb\nc\rd";
        let output = remove_line_breaks(input);
        assert_eq!(output, "abcd");
    }

    #[test]
    fn remove_line_breaks_keeps_text_without_line_breaks() {
        let input = "single line text";
        let output = remove_line_breaks(input);
        assert_eq!(output, "single line text");
    }

    #[test]
    fn remove_line_breaks_returns_empty_for_only_line_breaks() {
        let input = "\r\n\n\r";
        let output = remove_line_breaks(input);
        assert!(output.is_empty());
    }

    #[test]
    fn calculate_recording_duration_ms_handles_normal_and_zero_values() {
        let recording = RecordedAudio {
            samples: vec![0; 16_000],
            sample_rate: 16_000,
            channels: 1,
        };
        assert_eq!(calculate_recording_duration_ms(&recording), 1000);

        let zero_rate = RecordedAudio {
            samples: vec![0; 100],
            sample_rate: 0,
            channels: 1,
        };
        assert_eq!(calculate_recording_duration_ms(&zero_rate), 0);

        let zero_channels = RecordedAudio {
            samples: vec![0; 100],
            sample_rate: 16_000,
            channels: 0,
        };
        assert_eq!(calculate_recording_duration_ms(&zero_channels), 0);
    }
}
