use crate::processing;
use crate::recorder::RecordedAudio;
use crate::settings::SettingsStore;
use std::sync::mpsc;
use std::thread;

enum DispatchMessage {
    Process(RecordedAudio),
    Shutdown,
}

pub struct TranscriptionDispatcher {
    sender: mpsc::Sender<DispatchMessage>,
    worker: Option<thread::JoinHandle<()>>,
}

impl TranscriptionDispatcher {
    pub fn new(store: SettingsStore) -> Self {
        let (sender, receiver) = mpsc::channel::<DispatchMessage>();
        let worker = thread::spawn(move || {
            while let Ok(message) = receiver.recv() {
                match message {
                    DispatchMessage::Process(recording) => {
                        if let Err(err) = processing::handle_recording(&store, recording) {
                            #[cfg(debug_assertions)]
                            eprintln!("录音处理失败: {err}");
                            processing::emit_status("error");
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

impl Drop for TranscriptionDispatcher {
    fn drop(&mut self) {
        let _ = self.sender.send(DispatchMessage::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
