use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use base64::{engine::general_purpose, Engine as _};
use rand::RngCore;
use std::fs;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};
use tauri_plugin_store::StoreExt;
use thiserror::Error;

use super::types::*;
use super::validation::{
    normalize_sensevoice_settings, normalize_settings, validate_sensevoice_settings,
    validate_settings,
};

const SETTINGS_FILE: &str = "settings.json";
const SETTINGS_KEY_FILE: &str = "settings.key";
const SETTINGS_STORE_KEY: &str = "payload";
const HISTORY_STORE_KEY: &str = "transcriptionHistory";
const UPDATER_STATE_STORE_KEY: &str = "updaterState";

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("设置路径解析失败: {0}")]
    PathResolve(String),
    #[error("设置文件读写失败: {0}")]
    Io(String),
    #[error("设置加密解密失败: {0}")]
    Crypto(String),
    #[error("设置序列化失败: {0}")]
    Serde(String),
    #[error("设置存储操作失败: {0}")]
    Store(String),
}

#[derive(Clone)]
pub struct SettingsStore {
    app: AppHandle,
    write_lock: Arc<Mutex<()>>,
}

impl SettingsStore {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            write_lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn load(&self) -> Result<Settings, SettingsError> {
        let store = self
            .app
            .store(SETTINGS_FILE)
            .map_err(|err| SettingsError::Store(err.to_string()))?;
        let Some(payload) = store.get(SETTINGS_STORE_KEY) else {
            // No settings stored yet — persist defaults directly to avoid
            // calling save() which would deadlock when load() is invoked
            // from save_sensevoice (write_lock already held).
            let settings = normalize_settings(&Settings::default());
            let _ = self.persist_settings(&settings);
            return Ok(settings);
        };
        let encoded = payload.as_str().ok_or_else(|| {
            SettingsError::Serde("settings payload has invalid format".to_string())
        })?;
        let key = self.load_or_create_key()?;
        let decrypted = decrypt_payload(encoded, &key)?;
        match serde_json::from_str::<Settings>(&decrypted) {
            Ok(settings) => Ok(normalize_settings(&settings)),
            Err(err) => {
                eprintln!("[settings] 反序列化失败，将重置为默认设置: {err}");
                let settings = normalize_settings(&Settings::default());
                let _ = self.persist_settings(&settings);
                Ok(settings)
            }
        }
    }

    #[allow(dead_code)]
    pub fn save(&self, settings: &Settings) -> Result<(), SettingsError> {
        let _guard = self.write_lock.lock().unwrap_or_else(|e| e.into_inner());
        let normalized = normalize_settings(settings);
        validate_settings(&normalized)?;
        self.persist_settings(&normalized)
    }

    /// Save settings from the UI, preserving runtime-managed SenseVoice
    /// fields from the current persisted state.  The merge + persist happens
    /// under the write lock so concurrent runtime updates cannot be lost.
    pub fn save_user_settings(&self, settings: &Settings) -> Result<Settings, SettingsError> {
        let _guard = self.write_lock.lock().unwrap_or_else(|e| e.into_inner());
        let current = self.load()?;
        let mut merged = settings.clone();
        merged.sensevoice.enabled = current.sensevoice.enabled;
        merged.sensevoice.installed = current.sensevoice.installed;
        merged.sensevoice.download_state = current.sensevoice.download_state.clone();
        merged.sensevoice.last_error = current.sensevoice.last_error.clone();
        let normalized = normalize_settings(&merged);
        validate_settings(&normalized)?;
        self.persist_settings(&normalized)?;
        Ok(normalized)
    }

    pub fn load_sensevoice(&self) -> Result<SenseVoiceSettings, SettingsError> {
        let settings = self.load()?;
        Ok(settings.sensevoice)
    }

    pub fn save_sensevoice(&self, sensevoice: &SenseVoiceSettings) -> Result<(), SettingsError> {
        let _guard = self.write_lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut normalized = sensevoice.clone();
        normalize_sensevoice_settings(&mut normalized);
        validate_sensevoice_settings(&normalized)?;
        let mut settings = self.load()?;
        settings.sensevoice = normalized;
        self.persist_settings(&settings)
    }

    pub fn save_sensevoice_editable(
        &self,
        sensevoice: &SenseVoiceSettings,
    ) -> Result<(), SettingsError> {
        let _guard = self.write_lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut settings = self.load()?;
        let mut merged = settings.sensevoice.clone();
        merged.local_model = sensevoice.local_model.clone();
        merged.stop_mode = sensevoice.stop_mode.clone();
        merged.service_url = sensevoice.service_url.clone();
        merged.model_id = sensevoice.model_id.clone();
        merged.language = sensevoice.language.clone();
        merged.device = sensevoice.device.clone();
        normalize_sensevoice_settings(&mut merged);
        validate_sensevoice_settings(&merged)?;
        // Runtime-managed fields are preserved from the persisted settings and must not
        // be overwritten by the editable settings payload coming from the UI.
        settings.sensevoice = merged;
        self.persist_settings(&settings)
    }

    pub fn load_transcription_history(
        &self,
    ) -> Result<Vec<TranscriptionHistoryItem>, SettingsError> {
        let store = self
            .app
            .store(SETTINGS_FILE)
            .map_err(|err| SettingsError::Store(err.to_string()))?;
        let Some(payload) = store.get(HISTORY_STORE_KEY) else {
            return Ok(Vec::new());
        };
        let Some(encoded) = payload.as_str() else {
            return Ok(Vec::new());
        };
        let key = self.load_or_create_key()?;
        let decrypted = match decrypt_payload(encoded, &key) {
            Ok(value) => value,
            Err(_) => return Ok(Vec::new()),
        };
        match serde_json::from_str::<Vec<TranscriptionHistoryItem>>(&decrypted) {
            Ok(mut history) => {
                if history.len() > MAX_TRANSCRIPTION_HISTORY_ITEMS {
                    history.truncate(MAX_TRANSCRIPTION_HISTORY_ITEMS);
                }
                Ok(history)
            }
            Err(_) => Ok(Vec::new()),
        }
    }

    pub fn append_transcription_history(
        &self,
        item: TranscriptionHistoryItem,
    ) -> Result<(), SettingsError> {
        let mut history = self.load_transcription_history()?;
        history.insert(0, item);
        if history.len() > MAX_TRANSCRIPTION_HISTORY_ITEMS {
            history.truncate(MAX_TRANSCRIPTION_HISTORY_ITEMS);
        }
        self.persist_transcription_history(&history)
    }

    pub fn clear_transcription_history(&self) -> Result<(), SettingsError> {
        self.persist_transcription_history(&[])
    }

    pub fn load_deferred_update_version(&self) -> Result<Option<String>, SettingsError> {
        let store = self
            .app
            .store(SETTINGS_FILE)
            .map_err(|err| SettingsError::Store(err.to_string()))?;
        let Some(payload) = store.get(UPDATER_STATE_STORE_KEY) else {
            return Ok(None);
        };
        let Some(encoded) = payload.as_str() else {
            return Ok(None);
        };
        let key = self.load_or_create_key()?;
        let decrypted = match decrypt_payload(encoded, &key) {
            Ok(value) => value,
            Err(_) => return Ok(None),
        };
        let state = match serde_json::from_str::<UpdaterState>(&decrypted) {
            Ok(value) => value,
            Err(_) => return Ok(None),
        };
        Ok(state
            .deferred_version
            .filter(|value| !value.trim().is_empty()))
    }

    pub fn save_deferred_update_version(&self, version: Option<&str>) -> Result<(), SettingsError> {
        let deferred_version = version
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
        let state = UpdaterState { deferred_version };
        let json =
            serde_json::to_string(&state).map_err(|err| SettingsError::Serde(err.to_string()))?;
        let key = self.load_or_create_key()?;
        let encrypted = encrypt_payload(&json, &key)?;
        let store = self
            .app
            .store(SETTINGS_FILE)
            .map_err(|err| SettingsError::Store(err.to_string()))?;
        store.set(
            UPDATER_STATE_STORE_KEY.to_string(),
            serde_json::Value::String(encrypted),
        );
        store
            .save()
            .map_err(|err| SettingsError::Store(err.to_string()))?;
        Ok(())
    }

    fn persist_settings(&self, settings: &Settings) -> Result<(), SettingsError> {
        let json =
            serde_json::to_string(settings).map_err(|err| SettingsError::Serde(err.to_string()))?;
        let key = self.load_or_create_key()?;
        let encrypted = encrypt_payload(&json, &key)?;
        let store = self
            .app
            .store(SETTINGS_FILE)
            .map_err(|err| SettingsError::Store(err.to_string()))?;
        store.set(
            SETTINGS_STORE_KEY.to_string(),
            serde_json::Value::String(encrypted),
        );
        store
            .save()
            .map_err(|err| SettingsError::Store(err.to_string()))?;
        Ok(())
    }

    fn persist_transcription_history(
        &self,
        history: &[TranscriptionHistoryItem],
    ) -> Result<(), SettingsError> {
        let json =
            serde_json::to_string(history).map_err(|err| SettingsError::Serde(err.to_string()))?;
        let key = self.load_or_create_key()?;
        let encrypted = encrypt_payload(&json, &key)?;
        let store = self
            .app
            .store(SETTINGS_FILE)
            .map_err(|err| SettingsError::Store(err.to_string()))?;
        store.set(
            HISTORY_STORE_KEY.to_string(),
            serde_json::Value::String(encrypted),
        );
        store
            .save()
            .map_err(|err| SettingsError::Store(err.to_string()))?;
        Ok(())
    }

    fn load_or_create_key(&self) -> Result<[u8; 32], SettingsError> {
        let dir = self
            .app
            .path()
            .app_data_dir()
            .map_err(|err| SettingsError::PathResolve(err.to_string()))?;
        fs::create_dir_all(&dir).map_err(|err| SettingsError::Io(err.to_string()))?;
        let key_path = dir.join(SETTINGS_KEY_FILE);
        if key_path.exists() {
            let data =
                fs::read_to_string(&key_path).map_err(|err| SettingsError::Io(err.to_string()))?;
            let decoded = general_purpose::STANDARD
                .decode(data.trim())
                .map_err(|err| SettingsError::Crypto(err.to_string()))?;
            return decoded
                .as_slice()
                .try_into()
                .map_err(|_| SettingsError::Crypto("invalid key length".to_string()));
        }
        let mut key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut key);
        let encoded = general_purpose::STANDARD.encode(key);
        fs::write(&key_path, encoded).map_err(|err| SettingsError::Io(err.to_string()))?;
        Ok(key)
    }
}

fn encrypt_payload(plain: &str, key: &[u8; 32]) -> Result<String, SettingsError> {
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|err| SettingsError::Crypto(err.to_string()))?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let cipher_text = cipher
        .encrypt(nonce, plain.as_bytes())
        .map_err(|err| SettingsError::Crypto(err.to_string()))?;
    let mut combined = Vec::with_capacity(nonce_bytes.len() + cipher_text.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&cipher_text);
    Ok(general_purpose::STANDARD.encode(combined))
}

fn decrypt_payload(encoded: &str, key: &[u8; 32]) -> Result<String, SettingsError> {
    let decoded = general_purpose::STANDARD
        .decode(encoded)
        .map_err(|err| SettingsError::Crypto(err.to_string()))?;
    if decoded.len() < 12 {
        return Err(SettingsError::Crypto("cipher text too short".to_string()));
    }
    let (nonce_bytes, cipher_text) = decoded.split_at(12);
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|err| SettingsError::Crypto(err.to_string()))?;
    let nonce = Nonce::from_slice(nonce_bytes);
    let plain = cipher
        .decrypt(nonce, cipher_text)
        .map_err(|err| SettingsError::Crypto(err.to_string()))?;
    String::from_utf8(plain).map_err(|err| SettingsError::Crypto(err.to_string()))
}
