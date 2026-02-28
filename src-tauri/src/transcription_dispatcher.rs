use crate::processing;
use crate::recorder::RecordedAudio;
use crate::settings::{SettingsStore, TranscriptionHistoryItem, TranscriptionHistoryStatus};
use std::sync::mpsc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

enum DispatchMessage {
    Process(RecordedAudio),
    Shutdown,
}

pub struct TranscriptionDispatcher {
    sender: mpsc::Sender<DispatchMessage>,
    worker: Option<thread::JoinHandle<()>>,
}

impl TranscriptionDispatcher {
    pub fn new(app: AppHandle, store: SettingsStore) -> Self {
        let (sender, receiver) = mpsc::channel::<DispatchMessage>();
        let worker = thread::spawn(move || {
            while let Ok(message) = receiver.recv() {
                match message {
                    DispatchMessage::Process(recording) => {
                        let outcome = processing::handle_recording(&store, recording);
                        if !outcome.is_success() {
                            #[cfg(debug_assertions)]
                            {
                                if let Some(error_message) = outcome.error_message.as_ref() {
                                    eprintln!("录音处理失败: {error_message}");
                                }
                            }
                            processing::emit_status("error");
                        }

                        if !outcome.history_enabled {
                            continue;
                        }

                        let item = TranscriptionHistoryItem {
                            id: create_history_id(),
                            timestamp_ms: now_timestamp_ms(),
                            status: if outcome.is_success() {
                                TranscriptionHistoryStatus::Success
                            } else {
                                TranscriptionHistoryStatus::Failed
                            },
                            transcription_text: outcome.transcription_text,
                            final_text: outcome.final_text,
                            triggered: outcome.triggered,
                            triggered_by_keyword: outcome.triggered_by_keyword,
                            trigger_matches: outcome.trigger_matches,
                            error_message: outcome.error_message,
                        };

                        if let Err(err) = store.append_transcription_history(item.clone()) {
                            #[cfg(debug_assertions)]
                            eprintln!("写入历史记录失败: {err}");
                            continue;
                        }

                        if let Err(err) = app.emit("transcription-history-appended", &item) {
                            #[cfg(debug_assertions)]
                            eprintln!("发送历史记录事件失败: {err}");
                        }
                    }
                    DispatchMessage::Shutdown => break,
                }
            }
        });

        Self {
            sender,
            worker: Some(worker),
        }
    }

    pub fn enqueue(&self, recording: RecordedAudio) -> Result<(), String> {
        self.sender
            .send(DispatchMessage::Process(recording))
            .map_err(|_| "转写任务线程不可用".to_string())
    }
}

fn now_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn create_history_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("history-{timestamp}")
}

impl Drop for TranscriptionDispatcher {
    fn drop(&mut self) {
        let _ = self.sender.send(DispatchMessage::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
