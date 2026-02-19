pub mod client;
pub mod manager;
pub mod worker;

pub use manager::{SenseVoiceManager, SenseVoiceStatus};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SenseVoiceError {
    #[error("SenseVoice 配置错误: {0}")]
    Config(String),
    #[error("SenseVoice 请求失败: {0}")]
    Request(String),
    #[error("SenseVoice 响应解析失败: {0}")]
    Parse(String),
    #[error("SenseVoice 进程执行失败: {0}")]
    Process(String),
    #[error("SenseVoice 文件读写失败: {0}")]
    Io(String),
    #[error("SenseVoice URL 错误: {0}")]
    Url(String),
    #[error("SenseVoice 设置读写失败: {0}")]
    Settings(String),
}
